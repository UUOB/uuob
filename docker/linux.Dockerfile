# UUOB's Linux window (linux/, GTK 4 + Libadwaita): build and screenshot it.
# Libadwaita 1.5 or newer is needed (Debian 13, Ubuntu 24.04, Fedora 40).
#   docker build -t uuob-linux:1 -f docker/linux.Dockerfile docker
FROM rust:1-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
      libgtk-4-dev libadwaita-1-dev xvfb xauth imagemagick dbus-x11 fonts-inter adwaita-icon-theme \
    && rm -rf /var/lib/apt/lists/*
