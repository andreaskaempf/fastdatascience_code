#!/usr/bin/env python3
#
# Minimal OpenAI chat completion script, using OpenAI library.
# Set BASE_URL to point at the real OpenAI API or to a local reverse proxy.

import sys
import openai
from openai import OpenAI

# Point this at the local reverse proxy (e.g. "http://localhost:8000/v1")
# or at OpenAI directly ("https://api.openai.com/v1").
#BASE_URL = "https://api.openai.com/v1"
BASE_URL = "http://localhost:8000/v1"

# Cheapest model served by OpenAI's own API.
MODEL = "gpt-5-nano"

def chat_completion(prompt: str):

    # Create the client; assumes API key is defined in env OPENAI_API_KEY
    client = OpenAI(base_url=BASE_URL, timeout=60)

    # Attempt the request
    response = client.chat.completions.create(
        model=MODEL,
        messages=[{"role": "user", "content": prompt}],
    )

    # Return the full response, not just the answer
    return response

# Get prompt from command line, or use default
prompt = sys.argv[1] if len(sys.argv) == 2 else "What is a reverse proxy?"
print('Prompt:', prompt)

# Call the completion and split out answer and tokens
resp = chat_completion(prompt)
ans = resp.choices[0].message.content

# Show answer
#print(resp.model_dump())
print('Answer:', ans)
print('Usage:', resp.usage)
