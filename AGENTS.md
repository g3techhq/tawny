## Preview server

Before running browser UI checks, first check whether `http://127.0.0.1:8080` is already serving the app. Reuse an existing server when present. If it is unavailable, agents may start `dx serve --web --addr 127.0.0.1 --port 8080 --open false --interactive false` for the duration of the checks and must stop only the server they started afterward.
