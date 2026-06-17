#!/usr/bin/env python3
"""Call OpenRouter API with a prompt. Reads token from Hermes config.

Supports exponential-backoff retries for rate-limited models (e.g. Nemotron free).

Usage:
    call_openrouter.py [model] [prompt_file]
    echo "prompt" | call_openrouter.py [model]

Env vars:
    OR_MAX_RETRIES    Max retry attempts (default: 5)
    OR_BASE_DELAY     Base delay in seconds (default: 10)
    OR_TIMEOUT        Request timeout in seconds (default: 120)
"""
import json
import sys
import time
import yaml
import urllib.request
import urllib.error

# Load token
with open('/home/bzf/.hermes/config.yaml') as f:
    config = yaml.safe_load(f)
token = config['providers']['openrouter']['token']

# Config
model = sys.argv[1] if len(sys.argv) > 1 else "nvidia/nemotron-3-ultra-550b-a55b:free"
max_retries = int(sys.argv[2] if len(sys.argv) > 2 else "5")
base_delay = float(sys.argv[3] if len(sys.argv) > 3 else "10")
timeout = int(sys.argv[4] if len(sys.argv) > 4 else "120")

# Read prompt from stdin or file argument
prompt_file = None
if len(sys.argv) > 5:
    prompt_file = sys.argv[5]

if prompt_file:
    with open(prompt_file) as f:
        prompt = f.read()
else:
    prompt = sys.stdin.read()

# Build request
payload = {
    "model": model,
    "max_tokens": 16000,
    "messages": [{"role": "user", "content": prompt}]
}

last_error = None

for attempt in range(1, max_retries + 1):
    if attempt > 1:
        delay = base_delay * (2 ** (attempt - 2))  # 10, 20, 40, 80, ...
        print(f"Retry {attempt}/{max_retries} — waiting {delay}s...", file=sys.stderr)
        time.sleep(delay)

    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/chat/completions",
        data=json.dumps(payload).encode(),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json"
        }
    )

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            result = json.loads(resp.read())
            content = result['choices'][0]['message']['content']
            print(content)
            sys.exit(0)
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        last_error = f"HTTP {e.code}: {body[:500]}"
        # Check if it's a rate limit (429) — worth retrying
        if e.code in (429, 503, 504):
            print(f"[{e.code}] {body[:200]} — retrying...", file=sys.stderr)
            continue
        else:
            print(last_error, file=sys.stderr)
            sys.exit(1)
    except urllib.error.URLError as e:
        last_error = f"URLError: {e.reason}"
        print(f"{last_error} — retrying...", file=sys.stderr)
        continue
    except Exception as e:
        last_error = f"Error: {e}"
        print(f"{last_error} — retrying...", file=sys.stderr)
        continue

# All retries exhausted
print(f"GAVE UP after {max_retries} attempts. Last error: {last_error}", file=sys.stderr)
sys.exit(2)
