FROM node:22-bookworm-slim AS node-runtime

FROM python:3.13-slim-bookworm

ARG YTDLP_VERSION=2026.8.19
ARG POT_PROVIDER_VERSION=1.3.1

COPY --from=node-runtime /usr/local/bin/node /usr/local/bin/node

RUN python -m pip install --no-cache-dir \
    "yt-dlp[default]==${YTDLP_VERSION}" \
    "bgutil-ytdlp-pot-provider==${POT_PROVIDER_VERSION}" \
    && useradd --create-home --uid 10001 tawny

WORKDIR /app
COPY --chown=tawny:tawny server.py /app/server.py

USER tawny
ENV PORT=8080 \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1

EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=4s --start-period=10s --retries=6 \
    CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8080/health', timeout=3)"]

ENTRYPOINT ["python", "/app/server.py"]
