# Collaborative Keystone Web

React/Vite frontend for Collaborative Keystone.

## Local Development

```powershell
cd site\web
npm install
npm run dev
```

The web app defaults to `http://localhost:8080` for the API. To override it, copy `.env.example` to `.env.local` and set `VITE_API_BASE_URL`.

## Production Build

Set `VITE_API_BASE_URL` before building:

```powershell
$env:VITE_API_BASE_URL='https://api.collaborativekeystone.com'
npm run build
```

Serve the generated `dist` directory from the web service that Cloudflare Tunnel points at.
