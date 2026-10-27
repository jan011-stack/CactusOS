#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${CACTUS_VM_DIR:-$ROOT/.vm/debian12}"
IMAGE_URL="https://cloud.debian.org/images/cloud/bookworm/latest"
IMAGE_NAME="debian-12-genericcloud-amd64.qcow2"
IMAGE_PATH="$STATE_DIR/$IMAGE_NAME"
DISK_PATH="$STATE_DIR/cactusos.qcow2"
SEED_PATH="$STATE_DIR/seed.iso"
USER_DATA="$STATE_DIR/user-data"
KEY_PATH="$STATE_DIR/id_ed25519"
PID_PATH="$STATE_DIR/qemu.pid"
SERIAL_LOG="$STATE_DIR/serial.log"
SSH_PORT="${CACTUS_VM_SSH_PORT:-22222}"
VM_NAME="cactusos-debian12"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'Brak wymaganej komendy: %s\n' "$1" >&2
        exit 1
    fi
}

write_user_data() {
    local public_key
    public_key="$(cat "$KEY_PATH.pub")"
    python3 - "$public_key" "$ROOT/vm/svl_kernel.py" "$USER_DATA" <<'PY'
import base64
import json
import pathlib
import sys

public_key, source_path, output_path = sys.argv[1:]
program = pathlib.Path(source_path).read_bytes()
config = {
    "users": [
        {"name": "default"},
        {
            "name": "cactus",
            "groups": [],
            "shell": "/bin/bash",
            "lock_passwd": True,
            "sudo": False,
            "ssh_authorized_keys": [public_key],
        },
    ],
    "disable_root": True,
    "ssh_pwauth": False,
    "package_update": True,
    "packages": ["python3-minimal"],
    "write_files": [{
        "path": "/home/cactus/svl_kernel.py",
        "owner": "cactus:cactus",
        "permissions": "0644",
        "encoding": "b64",
        "content": base64.b64encode(program).decode("ascii"),
    }],
    "runcmd": [
        ["python3", "/home/cactus/svl_kernel.py"],
        ["sh", "-c", "python3 /home/cactus/svl_kernel.py > /home/cactus/svl-kernel-report.json"],
        ["chown", "cactus:cactus", "/home/cactus/svl-kernel-report.json"],
    ],
    "output": {"all": "| tee -a /var/log/cloud-init-output.log"},
}
pathlib.Path(output_path).write_text(
    "#cloud-config\n" + json.dumps(config, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
PY
}

is_running() {
    [[ -s "$PID_PATH" ]] || return 1
    local pid command
    pid="$(cat "$PID_PATH")"
    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    kill -0 "$pid" 2>/dev/null || return 1
    command="$(ps -p "$pid" -o args= 2>/dev/null || true)"
    [[ "$command" == *"$VM_NAME"* && "$command" == *"qemu-system-x86_64"* ]]
}

start_vm() {
    require_command curl
    require_command qemu-system-x86_64
    require_command qemu-img
    require_command cloud-localds
    require_command ssh
    require_command ssh-keygen
    require_command sudo
    sudo -n test -r /dev/kvm || {
        printf '%s\n' "Brak dostępu do /dev/kvm." >&2
        exit 1
    }

    mkdir -p "$STATE_DIR"
    chmod 700 "$STATE_DIR"
    if is_running; then
        printf 'VM działa. Raport: %s\n' "$(ssh_command 'cat /home/cactus/svl-kernel-report.json' 2>/dev/null || printf 'system się uruchamia')"
        return
    fi

    if [[ ! -s "$IMAGE_PATH" ]]; then
        local checksum
        printf 'Pobieram oficjalny obraz Debiana z %s/%s\n' "$IMAGE_URL" "$IMAGE_NAME"
        curl --fail --silent --show-error --location --retry 3 "$IMAGE_URL/SHA512SUMS" -o "$STATE_DIR/SHA512SUMS"
        checksum="$(awk -v image="$IMAGE_NAME" '$2 == image { print $1 }' "$STATE_DIR/SHA512SUMS")"
        [[ "$checksum" =~ ^[0-9a-f]{128}$ ]] || {
            printf '%s\n' "Nie znaleziono prawidłowej sumy SHA512 obrazu." >&2
            exit 1
        }
        curl --fail --silent --show-error --location --retry 3 "$IMAGE_URL/$IMAGE_NAME" -o "$IMAGE_PATH.part"
        printf '%s  %s\n' "$checksum" "$IMAGE_PATH.part" | sha512sum --check --status || {
            rm -f -- "$IMAGE_PATH.part"
            printf '%s\n' "Weryfikacja SHA512 obrazu Debiana nie powiodła się." >&2
            exit 1
        }
        mv -- "$IMAGE_PATH.part" "$IMAGE_PATH"
        chmod 644 "$IMAGE_PATH"
    fi

    if [[ ! -s "$KEY_PATH" ]]; then
        ssh-keygen -q -t ed25519 -N '' -C cactusos-vm -f "$KEY_PATH"
        chmod 600 "$KEY_PATH"
        chmod 644 "$KEY_PATH.pub"
    fi
    write_user_data
    cloud-localds "$SEED_PATH" "$USER_DATA"
    chmod 600 "$USER_DATA" "$SEED_PATH"
    if [[ ! -e "$DISK_PATH" ]]; then
        qemu-img create -f qcow2 -F qcow2 -b "$IMAGE_PATH" "$DISK_PATH" 12G
    fi
    rm -f -- "$PID_PATH"
    : > "$PID_PATH"
    : > "$SERIAL_LOG"
    chmod 600 "$PID_PATH" "$SERIAL_LOG"

    sudo -n qemu-system-x86_64 \
        -name "$VM_NAME" \
        -runas "$(id -un)" \
        -machine q35,accel=kvm \
        -cpu host \
        -smp 2 \
        -m 2048 \
        -nodefaults \
        -no-reboot \
        -display none \
        -monitor none \
        -serial "file:$SERIAL_LOG" \
        -pidfile "$PID_PATH" \
        -daemonize \
        -drive "if=virtio,format=qcow2,file=$DISK_PATH" \
        -drive "if=virtio,format=raw,readonly=on,file=$SEED_PATH" \
        -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
        -device virtio-net-pci,netdev=net0
    printf 'Uruchomiono QEMU/KVM, oczekuję na SSH (port %s)...\n' "$SSH_PORT"
    for _ in {1..120}; do
        if ! is_running; then
            printf '%s\n' "QEMU zakończyło działanie. Szczegóły:" >&2
            tail -80 "$SERIAL_LOG" >&2 || true
            exit 1
        fi
        if ssh_command 'true' >/dev/null 2>&1; then
            printf 'VM gotowa. Łączenie: %s ssh\n' "$0"
            ssh_command 'cat /home/cactus/svl-kernel-report.json'
            return
        fi
        sleep 2
    done
    printf 'VM działa, ale SSH jeszcze nie odpowiada. Log startowy: %s\n' "$SERIAL_LOG" >&2
    exit 1
}

ssh_command() {
    ssh -i "$KEY_PATH" \
        -o BatchMode=yes \
        -o ConnectTimeout=3 \
        -o StrictHostKeyChecking=accept-new \
        -o UserKnownHostsFile="$STATE_DIR/known_hosts" \
        -p "$SSH_PORT" \
        cactus@127.0.0.1 "$@"
}

case "${1:-start}" in
    start)
        start_vm
        ;;
    ssh)
        is_running || { printf '%s\n' "VM nie działa; uruchom: $0 start" >&2; exit 1; }
        shift
        ssh_command "$@"
        ;;
    status)
        if is_running; then
            printf 'VM działa (PID %s, SSH localhost:%s)\n' "$(cat "$PID_PATH")" "$SSH_PORT"
        else
            printf '%s\n' "VM nie działa."
            exit 1
        fi
        ;;
    stop)
        if is_running; then
            pid="$(cat "$PID_PATH")"
            kill -TERM "$pid"
            for _ in {1..10}; do
                kill -0 "$pid" 2>/dev/null || break
                sleep 1
            done
            if kill -0 "$pid" 2>/dev/null; then
                printf '%s\n' "QEMU nie zakończyło się po sygnale TERM; sprawdź status ręcznie." >&2
                exit 1
            fi
            rm -f -- "$PID_PATH"
            printf '%s\n' "VM zatrzymana; dysk i stan cloud-init pozostają na miejscu."
        else
            printf '%s\n' "VM już nie działa."
        fi
        ;;
    *)
        printf 'Użycie: %s [start|ssh|status|stop]\n' "$0" >&2
        exit 2
        ;;
esac
