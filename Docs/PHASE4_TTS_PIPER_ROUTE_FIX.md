# Piper HTTP route compatibility fix

Current Piper HTTP server versions expose the browser test page on `GET /`, the installed voice list on `GET /voices`, and synthesis on `POST /synthesize`.

FlowStation now treats `[tts].endpoint` as the provider base URL. Example:

```toml
[tts]
endpoint = "http://127.0.0.1:5005"
```

Internally FlowStation calls:

- `GET <endpoint>/voices` for availability checks
- `POST <endpoint>/synthesize` for WAV generation

For compatibility, an endpoint ending in `/synthesize` is also accepted and normalized back to the provider base URL before the two API routes are constructed.
