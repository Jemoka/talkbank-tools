# GUI-path verification: base prereqs + GUI extras per CONTRIBUTING.md.
FROM tb-verify:base

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -y \
 && apt-get install -y --no-install-recommends \
        libwebkit2gtk-4.1-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libssl-dev \
        patchelf \
        nodejs npm \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
