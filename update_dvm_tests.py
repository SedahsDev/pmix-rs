#!/usr/bin/env python3
"""
Update all DVM-ignored tests in pmix-rs to use daemon_helper::get_dvm_context()
and daemon_helper::daemon_lock() at the start of the function body.
"""
import re
import sys
from pathlib import Path

BASE = Path("/home/bzf/projects/pmix-rs")

FILES_NEEDING_MOD = [
    "tests/fabric_deep.rs",
    "tests/fabric_Load_topology.rs",
    "tests/groups_deep.rs",
    "tests/groups_Group_construct_nb.rs",
    "tests/groups_Group_construct.rs",
    "tests/groups_Group_destruct_nb.rs",
    "tests/groups_Group_invite_nb.rs",
    "tests/groups_lifecycle.rs",
    "tests/lib_core_lifecycle.rs",
]

TARGET_FILES = [
    "tests/cpu_locality_Get_cpuset.rs",
    "tests/cpu_locality_Get_relative_locality.rs",
    "tests/cpu_locality_Parse_cpuset_string.rs",
    "tests/data_ops_deep.rs",
    "tests/data_ops_Store_internal.rs",
    "tests/data_serialization_Compress.rs",
    "tests/data_serialization_Data_compress.rs",
    "tests/data_serialization_Data_copy_payload.rs",
    "tests/data_serialization_Data_copy.rs",
    "tests/data_serialization_Data_decompress.rs",
    "tests/data_serialization_Data_embed.rs",
    "tests/data_serialization_Data_print.rs",
    "tests/fabric_deep.rs",
    "tests/fabric_Load_topology.rs",
    "tests/groups_deep.rs",
    "tests/groups_Group_construct_nb.rs",
    "tests/groups_Group_construct.rs",
    "tests/groups_Group_destruct_nb.rs",
    "tests/groups_Group_invite_nb.rs",
    "tests/groups_lifecycle.rs",
    "tests/lib_core_lifecycle.rs",
]

# Regex for DVM-related ignore with message
DVM_IGNORE_MSG_RE = re.compile(
    r'^#\s*\[\s*ignore\s*=\s*"([^"]*(?:requires\s+(?:DVM|PMIx|server|daemon)|returns error without server|flaky[^"]*init)[^"]*)"\s*\]'
)

# Regex for bare #[ignore] (no message)
BARE_IGNORE_RE = re.compile(r'^#\s*\[\s*ignore\s*\]')

CTX_LINE = '    let _ctx = daemon_helper::get_dvm_context().expect("DVM context");'
LOCK_LINE = '    let _lock = daemon_helper::daemon_lock().expect("daemon lock");'

def add_mod_daemon_helper(filepath):
    """Add mod daemon_helper; after doc comments, before use statements."""
    fpath = BASE / filepath
    content = fpath.read_text()
    
    lines = content.split('\n')
    insert_idx = None
    
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith('//!') or stripped.startswith('//! '):
            continue
        elif stripped == '':
            continue
        elif stripped.startswith('mod daemon_helper'):
            return False  # Already has it
        else:
            insert_idx = i
            break
    
    if insert_idx is None:
        print(f"  WARNING: Could not find insertion point in {filepath}")
        return False
    
    lines.insert(insert_idx, 'mod daemon_helper;')
    fpath.write_text('\n'.join(lines))
    return True

def is_dvm_ignore(line):
    """Check if a line is a DVM-related ignore attribute (with or without message)."""
    return DVM_IGNORE_MSG_RE.match(line.strip()) is not None or BARE_IGNORE_RE.match(line.strip()) is not None

def process_file(filepath):
    """Process one file: find DVM-ignored tests and ensure they have the two lines."""
    fpath = BASE / filepath
    content = fpath.read_text()
    lines = content.split('\n')
    
    # Find all DVM-ignored test functions
    # Pattern: #[ignore ...] (with or without message) + fn ... {
    # The #[test] attribute may or may not be present
    test_functions = []
    
    i = 0
    while i < len(lines):
        if is_dvm_ignore(lines[i]):
            # Look ahead through attribute lines to find fn
            j = i + 1
            while j < len(lines):
                s = lines[j].strip()
                if s.startswith('#['):
                    # Another attribute (e.g. #[test]) - skip it
                    j += 1
                    continue
                elif s.startswith('fn '):
                    # Found fn line - check if { is on this line
                    if '{' in lines[j]:
                        test_functions.append(j)  # brace_line = j
                    else:
                        # Look for { on next lines
                        m = j + 1
                        while m < len(lines):
                            if '{' in lines[m]:
                                test_functions.append(m)
                                break
                            m += 1
                    break
                else:
                    # Not an attribute or fn - something unexpected
                    break
                j += 1
        i += 1
    
    if not test_functions:
        return 0
    
    # Process each test function
    modified_count = 0
    
    for brace_line in test_functions:
        # Check next few non-blank, non-comment lines after brace
        next_stmts = []
        for idx in range(brace_line + 1, min(brace_line + 10, len(lines))):
            s = lines[idx].strip()
            if s and not s.startswith('//'):
                next_stmts.append((idx, s))
            if len(next_stmts) >= 3:
                break
        
        has_ctx = any('daemon_helper::get_dvm_context()' in s for _, s in next_stmts)
        has_lock = any('daemon_helper::daemon_lock()' in s for _, s in next_stmts)
        
        if has_ctx and has_lock:
            # Already correct
            continue
        
        if not has_ctx and not has_lock:
            # Need to add both lines after brace
            lines.insert(brace_line + 1, LOCK_LINE)
            lines.insert(brace_line + 1, CTX_LINE)
            modified_count += 1
        elif has_ctx and not has_lock:
            # Has ctx but not lock - add lock after ctx
            for idx, s in next_stmts:
                if 'daemon_helper::get_dvm_context()' in s:
                    lines.insert(idx + 1, LOCK_LINE)
                    modified_count += 1
                    break
        elif has_lock and not has_ctx:
            # Has lock but not ctx - add ctx before lock
            for idx, s in next_stmts:
                if 'daemon_helper::daemon_lock()' in s:
                    lines.insert(idx, CTX_LINE)
                    modified_count += 1
                    break
    
    if modified_count > 0:
        fpath.write_text('\n'.join(lines))
    
    return modified_count

def count_dvm_ignores(filepath):
    """Count total DVM-ignored tests in a file."""
    fpath = BASE / filepath
    content = fpath.read_text()
    lines = content.split('\n')
    count = 0
    for line in lines:
        if is_dvm_ignore(line):
            count += 1
    return count

def main():
    # Step 1: Add mod daemon_helper; to files that need it
    print("Step 1: Adding mod daemon_helper; to files that need it...")
    for filepath in FILES_NEEDING_MOD:
        if add_mod_daemon_helper(filepath):
            print(f"  Added to {filepath}")
        else:
            print(f"  SKIPPED (already has it): {filepath}")
    
    # Step 2: Update all DVM-ignored tests
    print("\nStep 2: Updating DVM-ignored tests...")
    total = 0
    for filepath in TARGET_FILES:
        ignore_count = count_dvm_ignores(filepath)
        count = process_file(filepath)
        total += count
        status = f"modified ({count} tests, total ignores: {ignore_count})" if count > 0 else f"no changes (total ignores: {ignore_count})"
        print(f"  {filepath}: {status}")
    
    print(f"\nSummary:")
    print(f"  Files with mod daemon_helper; added: {len(FILES_NEEDING_MOD)}")
    print(f"  Total tests updated: {total}")
    
    return 0

if __name__ == '__main__':
    sys.exit(main())
