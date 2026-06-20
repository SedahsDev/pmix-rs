#!/usr/bin/env python3
"""
Refactor pmix-rs test files to use shared daemon_helper::get_dvm_context()
instead of per-file ensure_init() patterns, and remove DVM-related #[ignore] attributes.
"""
import re
import sys
from pathlib import Path

TESTS_DIR = Path("/home/bzf/projects/pmix-rs/tests")

# Files to skip entirely
SKIP_FILES = {
    "daemon_helper.rs",
    "init_via_prterun.rs",
}

def is_dvm_ignore(line: str) -> bool:
    """Check if a line is a DVM-related #[ignore] that should be removed."""
    s = line.strip()
    if not s.startswith('#[ignore'):
        return False
    # Check for DVM-related ignore reasons
    dvm_keywords = [
        'requires DVM-launched process',
        'requires prterun',
        'requires PMIx_Init',
        'requires PMIx runtime',
        'FFI calls fail without',
        'ensure_init calls PMIx_Init',
        'requires running daemon via prterun',
        'Requires PMIx runtime',
        'requires PMIx runtime',
    ]
    for kw in dvm_keywords:
        if kw in s:
            return True
    return False

def is_keep_ignore(line: str) -> bool:
    """Check if a line is a #[ignore] that should be kept."""
    s = line.strip()
    if not s.startswith('#[ignore'):
        return False
    keep_keywords = [
        'returns error without server',
        'requires mocking',
        'SIGSEGV',
        'requires valgrind',
        'undefined behavior',
        'released buffer',
        'requires server',
        'requires multiple processes',
        'requires spawn',
        'requires tool',
        'requires daemon',
        'requires PRTE',
        'requires server-specific',
        'not implemented in openpmix',
    ]
    for kw in keep_keywords:
        if kw in s:
            return True
    return False

def process_file(filepath: Path) -> tuple:
    """Process a single test file. Returns (modified, ignores_removed, errors)."""
    content = filepath.read_text()
    original_content = content
    errors = []
    
    lines = content.split('\n')
    new_lines = []
    
    # Track state
    in_once_lock_block = False
    once_lock_block_depth = 0
    skip_until_brace_close = False
    
    # Count DVM ignores before
    dvm_ignore_count = sum(1 for line in lines if is_dvm_ignore(line))
    
    # Check if we need to add mod daemon_helper
    has_mod_daemon_helper = any(
        line.strip() == 'mod daemon_helper;' for line in lines
    )
    
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        
        # Handle OnceLock block removal
        if not in_once_lock_block:
            # Check for start of OnceLock block
            if 'static PMIX_CTX' in stripped and 'OnceLock<pmix::Context>' in stripped:
                in_once_lock_block = True
                skip_until_brace_close = True
                i += 1
                continue
        
        if in_once_lock_block:
            if skip_until_brace_close:
                # Count braces to find end of block
                if '{' in stripped and '}' not in stripped:
                    # Opening brace found, look for closing
                    pass
                elif stripped == '{' or stripped.startswith('{'):
                    pass
                elif '}' in stripped:
                    # Check if this closes the function
                    # Look ahead to see if ensure_init has ended
                    in_once_lock_block = False
                    skip_until_brace_close = False
                    i += 1
                    continue
                i += 1
                continue
        
        # Remove DVM-related #[ignore] lines
        if is_dvm_ignore(stripped):
            i += 1
            continue
        
        # Remove unused OnceLock import
        if stripped == 'use std::sync::OnceLock;':
            i += 1
            continue
        
        # Remove 'init' from use pmix::{init, ...} imports
        if 'use pmix::' in stripped and 'init' in stripped and stripped.endswith(';'):
            # Carefully remove 'init' from the import list
            m = re.match(r'^(.*use\s+pmix::\{)\s*(init\s*,?\s*)(.*\})\s*;', line)
            if m:
                prefix = m.group(1)
                rest = m.group(3).strip()
                new_line = prefix + rest + '};'
                new_lines.append(new_line)
                i += 1
                continue
        
        # Replace ensure_init() calls
        if 'ensure_init()' in stripped:
            if stripped.startswith('let ') and stripped.endswith(';'):
                # Replace with daemon_helper call
                new_line = line.replace(
                    'ensure_init()',
                    'daemon_helper::get_dvm_context().expect("DVM context should be available under prterun")'
                )
                new_lines.append(new_line)
                i += 1
                continue
        
        # Replace direct pmix::init(None) calls
        if 'pmix::init(None)' in stripped:
            if stripped.startswith('let ') and '.expect(' in stripped and stripped.endswith(';'):
                new_line = re.sub(
                    r'let\s+\w+\s*=\s*pmix::init\(None\)\.expect\([^)]*\)\s*;',
                    'let _ctx = daemon_helper::get_dvm_context().expect("DVM context should be available under prterun");',
                    line
                )
                new_lines.append(new_line)
                i += 1
                continue
        
        new_lines.append(line)
        i += 1
    
    content = '\n'.join(new_lines)
    
    # Add mod daemon_helper; if needed
    if not has_mod_daemon_helper:
        # Find insertion point after doc comments
        insert_pos = 0
        for idx, line in enumerate(new_lines):
            s = line.strip()
            if s.startswith('//!') or s == '':
                insert_pos = idx + 1
            else:
                break
        new_lines.insert(insert_pos, 'mod daemon_helper;')
        content = '\n'.join(new_lines)
    
    # Count remaining DVM ignores
    remaining_dvm_ignores = sum(1 for line in content.split('\n') if is_dvm_ignore(line))
    ignores_removed = dvm_ignore_count - remaining_dvm_ignores
    
    modified = content != original_content
    if modified:
        filepath.write_text(content)
    
    return modified, ignores_removed, errors

def main():
    all_test_files = sorted(TESTS_DIR.glob("*.rs"))
    
    files_to_process = []
    for f in all_test_files:
        if f.name in SKIP_FILES:
            continue
        content = f.read_text()
        for line in content.split('\n'):
            if is_dvm_ignore(line):
                files_to_process.append(f)
                break
    
    print(f"Found {len(files_to_process)} files with DVM-related #[ignore] attributes")
    print("=" * 70)
    
    total_modified = 0
    total_ignores_removed = 0
    
    for filepath in files_to_process:
        modified, ignores_removed, errors = process_file(filepath)
        status = "MODIFIED" if modified else "unchanged"
        print(f"  {filepath.name}: {status}, {ignores_removed} DVM ignores removed")
        if errors:
            for e in errors:
                print(f"    ERROR: {e}")
        
        if modified:
            total_modified += 1
        total_ignores_removed += ignores_removed
    
    print("=" * 70)
    print(f"Summary: {total_modified} files modified, {total_ignores_removed} DVM #[ignore] attributes removed")

if __name__ == "__main__":
    main()
