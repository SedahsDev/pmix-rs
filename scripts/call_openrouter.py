#!/usr/bin/env python3
"""Call OpenRouter API with a prompt. Reads token from Hermes config."""
import json
import sys
import yaml
import urllib.request
import urllib.error

# Load token
with open('/home/bzf/.hermes/config.yaml') as f:
    config = yaml.safe_load(f)
token = config['providers']['openrouter']['token']

# Model defaults to Nemotron 3 Ultra (free fallback)
model = sys.argv[1] if len(sys.argv) > 1 else "nvidia/nemotron-3-ultra-550b-a5b:free"

# Read prompt from stdin or file argument
if len(sys.argv) > 2:
    with open(sys.argv[2]) as f:
        prompt = f.read()
else:
    prompt = sys.stdin.read()

# Build request
payload = {
    "model": model,
    "max_tokens": 16000,
    "messages": [{"role": "user", "content": prompt}]
}

req = urllib.request.Request(
    "https://openrouter.ai/api/v1/chat/completions",
    data=json.dumps(payload).encode(),
    headers={
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json"
    }
)

try:
    with urllib.request.urlopen(req, timeout=120) as resp:
        result = json.loads(resp.read())
        content = result['choices'][0]['message']['content']
        print(content)
        sys.exit(0)
except urllib.error.HTTPError as e:
    body = e.read().decode()
    print(f"HTTP {e.code}: {body[:500]}", file=sys.stderr)
    sys.exit(1)
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    sys.exit(2)
