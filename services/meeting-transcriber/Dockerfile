ARG PYTHON_RUNTIME_IMAGE=docker.io/library/python:3.12-slim-bookworm@sha256:d50fb7611f86d04a3b0471b46d7557818d88983fc3136726336b2a4c657aa30b
FROM ${PYTHON_RUNTIME_IMAGE}

ARG JIMIN_BUILD_SHA=dev
LABEL org.opencontainers.image.title="Jimin OS Meeting Transcriber" \
      org.opencontainers.image.source="https://github.com/Yaklede/jimin-os" \
      org.opencontainers.image.revision="${JIMIN_BUILD_SHA}"

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY services/meeting-transcriber/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir --requirement /app/requirements.txt
COPY services/meeting-transcriber/app.py /app/app.py

RUN useradd --create-home --uid 10002 transcriber \
    && mkdir -p /home/transcriber/.cache/huggingface /var/lib/jimin-transcriber/tmp/matplotlib \
    && chown -R 10002:10002 /home/transcriber /var/lib/jimin-transcriber
USER 10002:10002

ENV HF_HOME=/home/transcriber/.cache/huggingface \
    MPLCONFIGDIR=/var/lib/jimin-transcriber/tmp/matplotlib \
    TMPDIR=/var/lib/jimin-transcriber/tmp \
    TORCH_FORCE_NO_WEIGHTS_ONLY_LOAD=1

EXPOSE 8090
HEALTHCHECK --interval=15s --timeout=5s --start-period=15s --retries=8 \
  CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8090/healthz', timeout=3).read()"]
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8090", "--no-access-log"]
