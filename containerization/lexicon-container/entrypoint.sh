#!/usr/bin/env bash
set -euo pipefail

lc_echo() {
    echo "[[LEXICON-CONTAINER]] $1"
}

lc_echo "Lexicon is installed. Container is idle and staying alive."
lc_echo "Use podman exec -it <container> bash to work inside it, or podman exec <container> bash -c lexicon -V for one-off commands."

exec tail -f /dev/null
