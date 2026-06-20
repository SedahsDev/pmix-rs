#!/usr/bin/env python3
"""Remove #[ignore] attributes from daemon-dependent tests.

Keeps #[ignore] on tests that genuinely cannot run even with a daemon:
- AddressSanitizer/valgrind
- mocking
- multiple PMIx servers
- DVM-launched process (not external tool)
- PMIx server that does not support
- returns error without server
"""
import re
import sys
import os

# Patterns that should KEEP the #[ignore]
KEEP_PATTERNS = [
    r'AddressSanitizer',
    r'valgrind',
    r'mocking',
    r'multiple PMIx server',
    r'DVM-launched process',
    r'PMIx server that does not support',
    r'returns error without server',
]

def should_keep_ignore(line):
    """Check if this #[ignore] should be kept."""
    for pattern in KEEP_PATTERNS:
        if re.search(pattern, line, re.IGNORECASE):
            return True
    return False

def process_file(filepath):
    """Remove daemon-dependent #[ignore] annotations from a test file."""
    with open(filepath, 'r') as f:
        lines = f.readlines()

    new_lines = []
    removed = 0
    kept = 0

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.rstrip('\n')

        # Match #[ignore] or #[ignore = "..."]
        if re.match(r'^#\[ignore', stripped):
            if should_keep_ignore(stripped):
                new_lines.append(line)
                kept += 1
            else:
                # Remove this line
                removed += 1
        else:
            new_lines.append(line)

        i += 1

    if removed > 0 or kept > 0:
        with open(filepath, 'w') as f:
            f.writelines(new_lines)

    return removed, kept

def main():
    tests_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'tests')

    total_removed = 0
    total_kept = 0

    for fname in sorted(os.listdir(tests_dir)):
        if not fname.endswith('.rs'):
            continue
        filepath = os.path.join(tests_dir, fname)
        removed, kept = process_file(filepath)
        if removed > 0 or kept > 0:
            print(f"{fname}: removed={removed}, kept={kept}")
            total_removed += removed
            total_kept += kept

    print(f"\nTotal: removed={total_removed}, kept={total_kept}")

if __name__ == '__main__':
    main()
