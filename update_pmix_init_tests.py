#!/usr/bin/env python3
"""
Update ignored tests in pmix-rs that require PMIx_Init to use pmix::init(None).

This script:
1. Scans all .rs files in tests/
2. Finds tests with #[ignore] where the reason involves PMIx_Init, DVM, prterun,
   SIGSEGV/segfault (without init), or PMIx runtime/init — but NOT server-side init
3. For each such test, adds `let _ctx = pmix::init(None).expect("pmix::init failed");`
   as the first line of the test function body (if not already present)
4. Changes the ignore message to standardized form
5. Writes the modified files back
"""

import re
import os
import sys
from pathlib import Path

TESTS_DIR = Path("/home/bzf/projects/pmix-rs/tests")

# Patterns that indicate a test needs PMIx_Init (client-side init)
# These are matched against the ignore reason string AND surrounding comments
CLIENT_INIT_KEYWORDS = [
    r"requires\s+PMIx_Init",
    r"requires\s+DVM",
    r"requires\s+prterun",
    r"SIGSEGV.*(?:without\s+init|without\s+PMIx\s+init|calls\s+FFI\s+without\s+init)",
    r"segfaults?\s+without\s+(?:PMIx\s+)?init",
    r"requires\s+PMIx\s+runtime",
    r"requires\s+PMIx\s+initialization",
    r"PMIx_Disconnect_nb\s+segfaults",
]

# Patterns that indicate a test needs SERVER-side init — DO NOT TOUCH
SERVER_INIT_KEYWORDS = [
    r"requires\s+PMIx\s+server",
    r"requires\s+PMIx_server_init",
    r"requires\s+PMIx\s+daemon",
    r"daemon\s+isolation",
    r"returns\s+error\s+without\s+server",
    r"multiple\s+PMIx\s+servers",
    r"server\s+initialized",
    r"server\s+runtime",
    r"server\s+environment",
    r"server\s+to\s+be\s+initialized",
    r"server\s+initialization",
    r"server\s+with\s+attach",
    r"server\s+that\s+does\s+not\s+support",
    r"server\s+initialized\s+and\s+event",
    r"server\s+initialized\s+—",
    r"server\s+initialized\s+-",
]

NEW_IGNORE_MSG = "requires DVM-launched process (prterun)"
INIT_LINE = 'let _ctx = pmix::init(None).expect("pmix::init failed");'

def is_client_init_test(ignore_reason: str, doc_comments: str) -> bool:
    """Check if this test needs client-side PMIx_Init (not server init)."""
    # Check if it matches server patterns — if so, skip
    for pattern in SERVER_INIT_KEYWORDS:
        if re.search(pattern, ignore_reason, re.IGNORECASE):
            return False
        if re.search(pattern, doc_comments, re.IGNORECASE):
            return False

    # Check if it matches client patterns
    for pattern in CLIENT_INIT_KEYWORDS:
        if re.search(pattern, ignore_reason, re.IGNORECASE):
            return True
        if re.search(pattern, doc_comments, re.IGNORECASE):
            return True

    return False

def already_has_init(func_body: str) -> bool:
    """Check if the function body already contains pmix::init."""
    return bool(re.search(r'pmix::init\s*\(', func_body))

def find_test_functions(content: str):
    """
    Find all test functions with their #[ignore] attributes and body.
    Returns list of tuples: (match_object, ignore_line, ignore_line_start, ignore_line_end)
    """
    results = []
    lines = content.split('\n')

    i = 0
    while i < len(lines):
        line = lines[i]

        # Look for #[ignore] patterns
        ignore_match = None
        ignore_type = None

        # Pattern 1: #[ignore = "..."]
        m = re.match(r'^(\s*)#\[ignore\s*=\s*"([^"]*)"\s*\]', line)
        if m:
            ignore_match = m
            ignore_type = 'string'

        # Pattern 2: #[ignore] // requires PMIx_Init ...
        if not ignore_match:
            m = re.match(r'^(\s*)#\[ignore\]\s*//\s*(.*)', line)
            if m:
                ignore_match = m
                ignore_type = 'comment'

        # Pattern 3: #[ignore] (bare) — need to check surrounding comments
        if not ignore_match:
            m = re.match(r'^(\s*)#\[ignore\]\s*$', line)
            if m:
                ignore_match = m
                ignore_type = 'bare'

        if ignore_match:
            # Collect doc comments preceding this #[ignore]
            doc_comments = ""
            # Look backwards for doc comments (/// ...)
            j = i - 1
            comment_lines = []
            while j >= 0:
                stripped = lines[j].strip()
                if stripped.startswith('///') or stripped.startswith('// '):
                    comment_lines.insert(0, stripped)
                    j -= 1
                elif stripped == '' or stripped.startswith('//!'):
                    # Module-level comments or blank lines — continue looking
                    j -= 1
                else:
                    break
            doc_comments = '\n'.join(comment_lines)

            # Find the #[test] attribute and function signature
            # Look for #[test] before #[ignore]
            test_attr_line = None
            k = i - 1
            while k >= 0:
                if '#[test]' in lines[k]:
                    test_attr_line = k
                    break
                elif lines[k].strip() and not lines[k].strip().startswith('#['):
                    break
                k -= 1

            if test_attr_line is None:
                i += 1
                continue

            # Find the function definition after #[ignore]
            func_start = None
            func_name = None
            k = i + 1
            while k < len(lines):
                fm = re.match(r'^(\s*)fn\s+(\w+)\s*\(', lines[k])
                if fm:
                    func_start = k
                    func_name = fm.group(2)
                    break
                # Skip any more attributes
                if lines[k].strip().startswith('#['):
                    k += 1
                    continue
                # Skip blank lines
                if lines[k].strip() == '':
                    k += 1
                    continue
                break

            if func_start is None:
                i += 1
                continue

            # Find the function body (the { })
            brace_line = func_start
            # The opening brace might be on the same line as fn or next line
            if '{' not in lines[func_start]:
                if func_start + 1 < len(lines) and '{' in lines[func_start + 1]:
                    brace_line = func_start + 1
                else:
                    i += 1
                    continue

            # Find the first line after {
            first_body_line = brace_line
            if lines[brace_line].strip() == '{':
                first_body_line = brace_line + 1
            else:
                # Opening brace is on the same line as fn
                first_body_line = brace_line + 1

            # Get the indentation of the function body
            # Use the indentation of the line after the opening brace
            if first_body_line < len(lines):
                body_indent = len(lines[first_body_line]) - len(lines[first_body_line].lstrip())
            else:
                body_indent = 4

            # Determine the ignore reason
            if ignore_type == 'string':
                ignore_reason = ignore_match.group(2)
            elif ignore_type == 'comment':
                ignore_reason = ignore_match.group(2)
            else:
                ignore_reason = ""

            results.append({
                'ignore_line': i,
                'ignore_type': ignore_type,
                'ignore_reason': ignore_reason,
                'doc_comments': doc_comments,
                'test_attr_line': test_attr_line,
                'func_line': func_start,
                'func_name': func_name,
                'first_body_line': first_body_line,
                'body_indent': body_indent,
                'brace_line': brace_line,
            })

        i += 1

    return results

def process_file(filepath: Path) -> str | None:
    """Process a single test file. Returns diff summary or None if no changes."""
    content = filepath.read_text()
    tests = find_test_functions(content)

    if not tests:
        return None

    lines = content.split('\n')
    changes_made = 0
    changes_details = []

    # Process in reverse order to preserve line numbers
    for test_info in reversed(tests):
        ignore_reason = test_info['ignore_reason']
        doc_comments = test_info['doc_comments']

        if not is_client_init_test(ignore_reason, doc_comments):
            continue

        # This test needs client init — update it
        ignore_line = test_info['ignore_line']
        ignore_type = test_info['ignore_type']
        first_body_line = test_info['first_body_line']
        body_indent = test_info['body_indent']
        func_name = test_info['func_name']

        # Update the ignore message
        if ignore_type == 'string':
            lines[ignore_line] = f'#[ignore = "{NEW_IGNORE_MSG}"]'
        elif ignore_type == 'comment':
            lines[ignore_line] = f'#[ignore = "{NEW_IGNORE_MSG}"]'
        else:  # bare #[ignore]
            lines[ignore_line] = f'#[ignore = "{NEW_IGNORE_MSG}"]'

        # Check if we need to add pmix::init(None)
        # Extract the function body to check for existing init
        # Find the end of the function (matching braces)
        brace_line = test_info['brace_line']
        func_body_start = brace_line

        # Find function end by counting braces
        brace_count = 0
        func_end = None
        for k in range(brace_line, len(lines)):
            for ch in lines[k]:
                if ch == '{':
                    brace_count += 1
                elif ch == '}':
                    brace_count -= 1
                    if brace_count == 0:
                        func_end = k
                        break
            if func_end is not None:
                break

        if func_end is not None:
            func_body = '\n'.join(lines[func_body_start:func_end + 1])
        else:
            func_body = '\n'.join(lines[func_body_start:])

        if not already_has_init(func_body):
            # Add init line as first line of function body
            indent = ' ' * body_indent
            init_line = f'{indent}{INIT_LINE}'

            # Determine where to insert
            if lines[brace_line].strip() == '{':
                # Insert after the opening brace line
                insert_at = brace_line + 1
                lines.insert(insert_at, init_line)
                # Adjust all subsequent line numbers
                test_info['first_body_line'] += 1
            else:
                # Opening brace is on the fn line — insert on the next line
                insert_at = first_body_line
                lines.insert(insert_at, init_line)

        changes_made += 1
        changes_details.append(f"  - {func_name}")

    if changes_made:
        new_content = '\n'.join(lines)
        return f"Modified {filepath.name}: {changes_made} test(s)\n" + '\n'.join(changes_details)
    return None

def main():
    all_results = []
    total_changes = 0

    for rs_file in sorted(TESTS_DIR.glob("*.rs")):
        result = process_file(rs_file)
        if result:
            all_results.append(result)
            # Count tests modified
            total_changes += result.count("test(s)")
            # Write back
            content = rs_file.read_text()
            tests = find_test_functions(content)
            lines = content.split('\n')

            # Re-process for writing (we need to actually modify and write)
            # Since process_file already computed changes, we need to re-run
            pass

    # Actually, let me restructure: process_file should return the new content
    # Let me rewrite this more cleanly

    print("Processing test files...")
    total_modified = 0
    total_tests = 0

    for rs_file in sorted(TESTS_DIR.glob("*.rs")):
        content = rs_file.read_text()
        tests = find_test_functions(content)

        if not tests:
            continue

        lines = content.split('\n')
        changes = 0
        modified_tests = []

        # Process in reverse order to preserve line numbers
        for test_info in reversed(tests):
            ignore_reason = test_info['ignore_reason']
            doc_comments = test_info['doc_comments']

            if not is_client_init_test(ignore_reason, doc_comments):
                continue

            ignore_line = test_info['ignore_line']
            ignore_type = test_info['ignore_type']
            brace_line = test_info['brace_line']
            body_indent = test_info['body_indent']
            func_name = test_info['func_name']

            # Update ignore message
            if ignore_type == 'string':
                lines[ignore_line] = f'#[ignore = "{NEW_IGNORE_MSG}"]'
            elif ignore_type == 'comment':
                lines[ignore_line] = f'#[ignore = "{NEW_IGNORE_MSG}"]'
            else:
                lines[ignore_line] = f'#[ignore = "{NEW_IGNORE_MSG}"]'

            # Check if init already exists
            brace_count = 0
            func_end = None
            for k in range(brace_line, len(lines)):
                for ch in lines[k]:
                    if ch == '{':
                        brace_count += 1
                    elif ch == '}':
                        brace_count -= 1
                        if brace_count == 0:
                            func_end = k
                            break
                if func_end is not None:
                    break

            func_body = '\n'.join(lines[brace_line:func_end + 1]) if func_end else '\n'.join(lines[brace_line:])

            if not already_has_init(func_body):
                indent = ' ' * body_indent
                init_line = f'{indent}{INIT_LINE}'

                if lines[brace_line].strip() == '{':
                    lines.insert(brace_line + 1, init_line)
                else:
                    first_body_line = test_info['first_body_line']
                    lines.insert(first_body_line, init_line)

            changes += 1
            modified_tests.append(func_name)

        if changes:
            new_content = '\n'.join(lines)
            rs_file.write_text(new_content)
            print(f"  {rs_file.name}: {changes} test(s) modified")
            for t in modified_tests:
                print(f"    - {t}")
            total_modified += 1
            total_tests += changes

    print(f"\nSummary: {total_modified} files modified, {total_tests} tests updated")

if __name__ == "__main__":
    main()
