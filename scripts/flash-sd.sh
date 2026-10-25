#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Build the SD boot prefix and write it to a card. The prefix only replaces the
# MBR and sector 16; the existing FAT partition is left intact. With --format
# and/or --copy-payload the script also recreates the partition filesystem and
# puts payload.bin in its root. Runbook: docs/agents/hil.md.
set -euo pipefail

cd "$(dirname "$0")/.."

SD_DEV=${SD_DEV:-}
SD_PART=${SD_PART:-}
START_LBA=${START_LBA:-}
PARTITION_SECTORS=${PARTITION_SECTORS:-}
LOADER_IMG=${LOADER_IMG:-target/rawboot-boot-sd.bin}
PAYLOAD_IMG=${PAYLOAD_IMG:-target/rawboot-payload.img}
BOOT_CFG=${BOOT_CFG:-}
PAYLOAD_NAME=${PAYLOAD_NAME:-payload.bin}
FORMAT_PARTITION=${FORMAT_PARTITION:-0}
COPY_PAYLOAD=${COPY_PAYLOAD:-}
SKIP_BUILD=${SKIP_BUILD:-0}
ASSUME_YES=${ASSUME_YES:-0}
DRY_RUN=${DRY_RUN:-0}
ALLOW_FILE=${ALLOW_FILE:-0}
LIST_DEVICES=0

PREFIX_IMG=target/rawboot-sd-mbr.img
TMP_MOUNT=

usage() {
    cat <<'EOF'
usage: scripts/flash-sd.sh [--device <dev>] [options]

Write target/rawboot-sd-mbr.img (MBR + SD loader at sector 16) onto a card that
already has a FAT partition starting at the same LBA. Without --device the script
looks for a single removable/USB/MMC disk and asks when there are several. Run as
your user, not with sudo: it escalates only dd/mkfs/mount, so cargo keeps target/
user-owned.

Arguments:
  --device <dev>            block device, e.g. /dev/sdX (or SD_DEV); default:
                            auto-detect
  --list                    list candidate disks and exit
  --partition <dev>         partition device; default <dev>1 / <dev>p1
  --start-lba <n>           partition start; default: read from the partition
                            (file mode: 2048)
  --partition-sectors <n>   partition size; default: read from the partition
  --loader <path>           sealed SD loader (default target/rawboot-boot-sd.bin)
  --payload <path>          payload image (default target/rawboot-payload.img)
  --payload-name <name>     file name on the partition (default payload.bin)
  --boot-cfg <path>         also copy this file as boot.cfg
  --format                  mkfs.vfat -F 32 the partition (DESTROYS its contents)
  --copy-payload            mount the partition and copy the payload image
  --no-copy-payload         do not copy payload.bin
  --skip-build              do not run `cargo xtask image`/`payload`
  --yes                     do not ask for confirmation
  --dry-run                 print the plan; do not build, format or write
  --allow-file              allow a regular file (tests; no format/payload copy)
  -h, --help                show this help

Environment: SD_DEV, SD_PART, START_LBA, PARTITION_SECTORS, LOADER_IMG,
PAYLOAD_IMG, BOOT_CFG, PAYLOAD_NAME, FORMAT_PARTITION, COPY_PAYLOAD,
SKIP_BUILD, ASSUME_YES, DRY_RUN, ALLOW_FILE (flags override them).

Examples:
  scripts/flash-sd.sh --list
  scripts/flash-sd.sh --copy-payload
  scripts/flash-sd.sh --device /dev/sdX --format --yes
  scripts/flash-sd.sh --dry-run
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

need() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is required but was not found"
}

show() {
    printf '+' >&2
    printf ' %q' "$@" >&2
    printf '\n' >&2
}

run() {
    if [ "$DRY_RUN" = "1" ]; then
        show "$@"
    else
        "$@"
    fi
}

is_uint() {
    case "$1" in
        '' | *[!0-9]*) return 1 ;;
        *) return 0 ;;
    esac
}

need_value() {
    [ "$#" -ge 2 ] || die "$1 needs a value"
}

root_disk_name() {
    local root_src root_disk
    root_src=$(findmnt -n -o SOURCE / 2>/dev/null) || return 1
    root_disk=$(lsblk -sno NAME "$root_src" 2>/dev/null | tail -n 1 | tr -d '[:space:]') || return 1
    [ -n "$root_disk" ] || return 1
    printf '%s\n' "$root_disk"
}

# Whole removable, USB or MMC disks that are not mounted and do not hold /.
sd_candidates() {
    local root_disk line name rm tran type size dev mounted model
    root_disk=$(root_disk_name) || return 1
    while IFS= read -r line; do
        [[ $line =~ ^NAME=\"([^\"]+)\"[[:space:]]+RM=\"([^\"]*)\"[[:space:]]+TRAN=\"([^\"]*)\"[[:space:]]+TYPE=\"([^\"]*)\"[[:space:]]+SIZE=\"([^\"]*)\" ]] ||
            continue
        name=${BASH_REMATCH[1]}
        rm=${BASH_REMATCH[2]}
        tran=${BASH_REMATCH[3]}
        type=${BASH_REMATCH[4]}
        size=${BASH_REMATCH[5]}
        [ "$type" = "disk" ] || continue
        case "$name" in
            loop* | zram* | dm-* | sr*) continue ;;
        esac
        if [ "$rm" != "1" ] && [ "$tran" != "usb" ] && [ "$tran" != "mmc" ]; then
            continue
        fi
        [ "$name" != "$root_disk" ] || continue
        dev="/dev/$name"
        mounted=$(lsblk -lnpo MOUNTPOINT "$dev" 2>/dev/null) || continue
        if printf '%s' "$mounted" | grep -q '[^[:space:]]'; then
            continue
        fi
        model=$(cat "/sys/block/$name/device/model" 2>/dev/null || true)
        printf '%s\t%s\t%s\t%s\t%s\n' "$name" "$size" "$tran" "$rm" "$model"
    done < <(lsblk -dno NAME,RM,TRAN,TYPE,SIZE -P 2>/dev/null)
}

print_candidates() {
    local i=1 line name size tran rm model
    for line in "$@"; do
        IFS=$'\t' read -r name size tran rm model <<< "$line"
        if [ "$rm" = "1" ]; then
            printf '  %d) /dev/%s  %s  %s  removable  %s\n' "$i" "$name" "$size" "$tran" "$model"
        else
            printf '  %d) /dev/%s  %s  %s  %s\n' "$i" "$name" "$size" "$tran" "$model"
        fi
        i=$((i + 1))
    done
}

choose_device() {
    local output choice index name size tran rm model
    need lsblk
    need findmnt
    output=$(sd_candidates) || die "cannot enumerate disks; pass --device /dev/sdX"
    CANDIDATES=()
    if [ -n "$output" ]; then
        mapfile -t CANDIDATES <<< "$output"
    fi
    if [ "${#CANDIDATES[@]}" -eq 0 ]; then
        die "no removable/USB/MMC disk found; connect the card or pass --device /dev/sdX"
    fi
    if [ "${#CANDIDATES[@]}" -eq 1 ]; then
        IFS=$'\t' read -r name size tran rm model <<< "${CANDIDATES[0]}"
        SD_DEV=/dev/$name
        echo "detected $SD_DEV ($size, $tran)" >&2
        return 0
    fi

    echo "found ${#CANDIDATES[@]} candidate disks:" >&2
    print_candidates "${CANDIDATES[@]}" >&2
    if [ ! -t 0 ]; then
        die "multiple candidates; pass --device /dev/sdX"
    fi
    printf 'Select 1-%d [1]: ' "${#CANDIDATES[@]}"
    read -r choice
    choice=${choice:-1}
    is_uint "$choice" || die "invalid selection: $choice"
    index=$((choice - 1))
    [ "$index" -ge 0 ] && [ "$index" -lt "${#CANDIDATES[@]}" ] || die "invalid selection: $choice"
    IFS=$'\t' read -r name size tran rm model <<< "${CANDIDATES[$index]}"
    SD_DEV=/dev/$name
}

confirm() {
    local answer
    if [ "$ASSUME_YES" = "1" ]; then
        return 0
    fi
    if [ "$DRY_RUN" = "1" ]; then
        return 0
    fi
    [ -t 0 ] || die "refusing without a TTY; pass --yes"
    printf '%s [y/N] ' "$1"
    read -r answer
    case "$answer" in
        y | Y | yes | YES) ;;
        *) die "aborted" ;;
    esac
}

cleanup() {
    if [ -n "$TMP_MOUNT" ]; then
        if [ "${EUID:-$(id -u)}" -eq 0 ]; then
            umount "$TMP_MOUNT" || true
        else
            sudo umount "$TMP_MOUNT" || true
        fi
        rmdir "$TMP_MOUNT" || true
    fi
}
trap cleanup EXIT

while [ "$#" -gt 0 ]; do
    case "$1" in
        --device)
            need_value "$@"
            SD_DEV=$2
            shift 2
            ;;
        --partition)
            need_value "$@"
            SD_PART=$2
            shift 2
            ;;
        --start-lba)
            need_value "$@"
            START_LBA=$2
            shift 2
            ;;
        --partition-sectors)
            need_value "$@"
            PARTITION_SECTORS=$2
            shift 2
            ;;
        --loader)
            need_value "$@"
            LOADER_IMG=$2
            shift 2
            ;;
        --payload)
            need_value "$@"
            PAYLOAD_IMG=$2
            shift 2
            ;;
        --payload-name)
            need_value "$@"
            PAYLOAD_NAME=$2
            shift 2
            ;;
        --boot-cfg)
            need_value "$@"
            BOOT_CFG=$2
            shift 2
            ;;
        --format)
            FORMAT_PARTITION=1
            shift
            ;;
        --copy-payload)
            COPY_PAYLOAD=1
            shift
            ;;
        --no-copy-payload)
            COPY_PAYLOAD=0
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --yes)
            ASSUME_YES=1
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --allow-file)
            ALLOW_FILE=1
            shift
            ;;
        --list)
            LIST_DEVICES=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            die "unknown argument: $1"
            ;;
    esac
done

if [ "$LIST_DEVICES" = "1" ]; then
    need lsblk
    need findmnt
    output=$(sd_candidates) || die "cannot enumerate disks"
    if [ -z "$output" ]; then
        echo "no removable/USB/MMC disk found"
        exit 0
    fi
    mapfile -t CANDIDATES <<< "$output"
    echo "candidate disks:"
    print_candidates "${CANDIDATES[@]}"
    exit 0
fi

if [ -z "$SD_DEV" ]; then
    choose_device
fi

if [ -z "$COPY_PAYLOAD" ]; then
    if [ "$FORMAT_PARTITION" = "1" ] || [ -n "$BOOT_CFG" ]; then
        COPY_PAYLOAD=1
    else
        COPY_PAYLOAD=0
    fi
fi

need readlink
need stat
need dd

SD_DEV_INPUT=$SD_DEV
[ -e "$SD_DEV_INPUT" ] ||
    die "device not found: $SD_DEV_INPUT; use --list or leave --device unset"
SD_DEV=$(readlink -f "$SD_DEV_INPUT")
IS_BLOCK=0
if [ -b "$SD_DEV" ]; then
    IS_BLOCK=1
fi
if [ "$IS_BLOCK" = "0" ] && [ "$ALLOW_FILE" != "1" ]; then
    die "$SD_DEV is not a block device; use --list/--device for a card, or --allow-file for tests"
fi

if [ "$IS_BLOCK" = "1" ]; then
    need lsblk
    need findmnt

    dev_name=$(basename "$SD_DEV")
    if [ -e "/sys/class/block/$dev_name/partition" ]; then
        die "$SD_DEV is a partition; pass the whole disk (for example /dev/sdX)"
    fi

    pttype=$(lsblk -dno PTTYPE "$SD_DEV" 2>/dev/null) ||
        die "cannot read the partition table type of $SD_DEV"
    pttype=$(printf '%s' "$pttype" | tr -d '[:space:]')
    case "$pttype" in
        dos) ;;
        '') die "$SD_DEV has no MBR; create a DOS/MBR FAT partition first" ;;
        *) die "$SD_DEV uses $pttype; the SD prefix writes an MBR, so use a DOS/MBR card" ;;
    esac

    part_types=$(lsblk -lnpo TYPE "$SD_DEV" 2>/dev/null) ||
        die "cannot list the partitions of $SD_DEV"
    part_count=$(printf '%s\n' "$part_types" | grep -c '^part$' || true)
    [ "$part_count" -eq 1 ] ||
        die "$SD_DEV must have exactly one partition, found $part_count"

    case "$dev_name" in
        mmcblk* | nvme* | loop*) default_part="${SD_DEV}p1" ;;
        *) default_part="${SD_DEV}1" ;;
    esac
    part_input=${SD_PART:-$default_part}
    [ -e "$part_input" ] || die "partition not found: $part_input"
    SD_PART=$(readlink -f "$part_input")
    [ -b "$SD_PART" ] || die "partition is not a block device: $SD_PART"

    parent=$(lsblk -no PKNAME "$SD_PART" 2>/dev/null | head -n 1 | tr -d '[:space:]') ||
        die "cannot find the parent disk of $SD_PART"
    [ -n "$parent" ] || die "cannot find the parent disk of $SD_PART"
    [ "$(readlink -f "/dev/$parent")" = "$SD_DEV" ] ||
        die "$SD_PART is not a partition of $SD_DEV"

    mounted=$(lsblk -lnpo MOUNTPOINT "$SD_DEV" 2>/dev/null) ||
        die "cannot inspect the mounts of $SD_DEV"
    if printf '%s' "$mounted" | grep -q '[^[:space:]]'; then
        die "$SD_DEV or one of its partitions is mounted; unmount it first"
    fi

    root_disk=$(root_disk_name) ||
        die "cannot determine the disk holding /; refusing to write"
    if [ "$(readlink -f "/dev/$root_disk")" = "$SD_DEV" ]; then
        die "refusing to overwrite $SD_DEV: it holds the root filesystem"
    fi

    part_name=$(basename "$SD_PART")
    detected_start=$(cat "/sys/class/block/$part_name/start" 2>/dev/null) ||
        die "cannot read the start LBA of $SD_PART"
    detected_start=$(printf '%s' "$detected_start" | tr -d '[:space:]')
    is_uint "$detected_start" || die "cannot read the start LBA of $SD_PART; pass --start-lba"
    [ "$detected_start" -gt 0 ] ||
        die "cannot read the start LBA of $SD_PART; pass --start-lba"
    if [ -n "$START_LBA" ]; then
        [ "$START_LBA" = "$detected_start" ] ||
            die "START_LBA=$START_LBA, but $SD_PART starts at $detected_start"
    else
        START_LBA=$detected_start
    fi

    detected_sectors=$(cat "/sys/class/block/$part_name/size" 2>/dev/null) ||
        die "cannot read the size of $SD_PART"
    detected_sectors=$(printf '%s' "$detected_sectors" | tr -d '[:space:]')
    if ! is_uint "$detected_sectors" || [ "$detected_sectors" -le 0 ]; then
        die "cannot read the size of $SD_PART"
    fi
    if [ -n "$PARTITION_SECTORS" ]; then
        [ "$PARTITION_SECTORS" = "$detected_sectors" ] ||
            die "PARTITION_SECTORS=$PARTITION_SECTORS, but $SD_PART has $detected_sectors"
    else
        PARTITION_SECTORS=$detected_sectors
    fi

    if [ "$FORMAT_PARTITION" != "1" ]; then
        fstype=$(lsblk -no FSTYPE "$SD_PART" 2>/dev/null) ||
            die "cannot read the filesystem type of $SD_PART"
        fstype=$(printf '%s' "$fstype" | tr -d '[:space:]')
        case "$fstype" in
            vfat | msdos | fat) ;;
            '') die "$SD_PART has no FAT filesystem; create one or use --format" ;;
            *) die "$SD_PART is '$fstype', not FAT; use --format (destroys its contents) or format it manually" ;;
        esac
    fi

    disk_sectors=$(cat "/sys/class/block/$dev_name/size" 2>/dev/null) ||
        die "cannot read the size of $SD_DEV"
    disk_sectors=$(printf '%s' "$disk_sectors" | tr -d '[:space:]')
    is_uint "$disk_sectors" || die "cannot read the size of $SD_DEV"
    if [ "$START_LBA" -gt "$disk_sectors" ]; then
        die "$SD_DEV has $disk_sectors sectors, but the prefix needs $START_LBA"
    fi

    removable=$(lsblk -dno RM "$SD_DEV" 2>/dev/null || true)
    if [ "$removable" = "0" ]; then
        echo "warning: $SD_DEV reports removable=0; check it is the card, not a system disk" >&2
    fi
else
    part_real=$(readlink -f "$SD_PART" 2>/dev/null || true)
    if [ -n "$SD_PART" ] && [ "$part_real" != "$SD_DEV" ]; then
        die "--partition is not used with --allow-file; the file itself is the target"
    fi
    if [ "$FORMAT_PARTITION" = "1" ] || [ "$COPY_PAYLOAD" = "1" ]; then
        die "--format/--copy-payload need a real block device and partition"
    fi
    SD_PART=$SD_DEV
    START_LBA=${START_LBA:-2048}
    if [ -z "$PARTITION_SECTORS" ]; then
        bytes=$(stat -c %s "$SD_DEV")
        PARTITION_SECTORS=$((bytes / 512))
    fi
fi

if ! is_uint "$START_LBA" || [ "$START_LBA" -le 0 ]; then
    die "start LBA must be a positive integer, got '$START_LBA'"
fi
if ! is_uint "$PARTITION_SECTORS" || [ "$PARTITION_SECTORS" -le 0 ]; then
    die "partition size must be a positive sector count, got '$PARTITION_SECTORS'"
fi
for name in FORMAT_PARTITION COPY_PAYLOAD SKIP_BUILD ASSUME_YES DRY_RUN ALLOW_FILE; do
    value=${!name}
    case "$value" in
        0 | 1) ;;
        *) die "$name must be 0 or 1, got '$value'" ;;
    esac
done
if [ -n "$BOOT_CFG" ] && [ "$COPY_PAYLOAD" != "1" ]; then
    die "--boot-cfg needs --copy-payload (or leave COPY_PAYLOAD unset)"
fi
case "$PAYLOAD_NAME" in
    '' | . | .. | */*) die "--payload-name must be a plain file name, got '$PAYLOAD_NAME'" ;;
esac
case "$(basename "$LOADER_IMG")" in
    rawboot-fel.bin)
        die "rawboot-fel.bin is linked at 0x00028000 and will not boot from sector 16; use rawboot-boot-sd.bin"
        ;;
esac

if [ "${EUID:-$(id -u)}" -eq 0 ]; then
    echo "warning: running as root; cargo builds create root-owned target/ files" >&2
    echo "warning: prefer running this script as your user; it uses sudo only for the device" >&2
fi

if [ "$DRY_RUN" != "1" ]; then
    need cargo
fi
if [ "$DRY_RUN" != "1" ] && [ "${EUID:-$(id -u)}" -ne 0 ]; then
    need sudo
fi
if [ "$FORMAT_PARTITION" = "1" ] && [ "$DRY_RUN" != "1" ]; then
    need mkfs.vfat
fi

if [ "$DRY_RUN" != "1" ] && [ "$SKIP_BUILD" != "1" ]; then
    run cargo xtask image
    run cargo xtask payload
fi
run cargo xtask sd-image \
    --partition-sectors "$PARTITION_SECTORS" \
    --start-lba "$START_LBA" \
    --loader "$LOADER_IMG"

if [ "$DRY_RUN" != "1" ]; then
    [ -f "$LOADER_IMG" ] || die "loader image not found: $LOADER_IMG (run cargo xtask image)"
    [ -f "$PREFIX_IMG" ] || die "SD prefix not found: $PREFIX_IMG"
    expected=$((START_LBA * 512))
    actual=$(stat -c %s "$PREFIX_IMG")
    [ "$actual" -eq "$expected" ] ||
        die "$PREFIX_IMG is $actual bytes, expected $expected for LBA $START_LBA"
    if [ "$COPY_PAYLOAD" = "1" ]; then
        [ -f "$PAYLOAD_IMG" ] || die "payload image not found: $PAYLOAD_IMG (run cargo xtask payload)"
    fi
    if [ -n "$BOOT_CFG" ] && [ "$COPY_PAYLOAD" = "1" ]; then
        [ -f "$BOOT_CFG" ] || die "boot.cfg not found: $BOOT_CFG"
    fi
fi

echo "device:     $SD_DEV"
echo "partition:  $SD_PART (LBA $START_LBA, $PARTITION_SECTORS sectors)"
echo "prefix:     $PREFIX_IMG"
if [ "$COPY_PAYLOAD" = "1" ]; then
    echo "payload:    $PAYLOAD_NAME <- $PAYLOAD_IMG"
fi
if [ "$FORMAT_PARTITION" = "1" ]; then
    echo "format:     $SD_PART as FAT32 (contents will be lost)"
fi

# Ask for every destructive action before writing anything, so declining a
# later step cannot leave a half-written card.
if [ "$FORMAT_PARTITION" = "1" ]; then
    confirm "Format $SD_PART as FAT32? This destroys its contents."
fi
if [ "$COPY_PAYLOAD" = "1" ]; then
    confirm "Write the SD boot prefix to $SD_DEV and copy $PAYLOAD_NAME to $SD_PART?"
else
    confirm "Write the SD boot prefix to $SD_DEV?"
fi

DD=(dd if="$PREFIX_IMG" of="$SD_DEV" bs=512 'conv=notrunc,fsync' status=progress)
if [ "${EUID:-$(id -u)}" -ne 0 ] && [ ! -w "$SD_DEV" ]; then
    DD=(sudo "${DD[@]}")
fi
run "${DD[@]}"
run sync

if [ "$FORMAT_PARTITION" = "1" ]; then
    MKFS=(mkfs.vfat -F 32 "$SD_PART")
    if [ "${EUID:-$(id -u)}" -ne 0 ]; then
        MKFS=(sudo "${MKFS[@]}")
    fi
    run "${MKFS[@]}"
    run sync
fi

if [ "$COPY_PAYLOAD" = "1" ]; then
    if [ "$DRY_RUN" = "1" ]; then
        show mount "$SD_PART" "<temporary directory>"
        show cp "$PAYLOAD_IMG" "<mount>/$PAYLOAD_NAME"
        if [ -n "$BOOT_CFG" ]; then
            show cp "$BOOT_CFG" "<mount>/boot.cfg"
        fi
        show umount "<temporary directory>"
    else
        TMP_MOUNT=$(mktemp -d "${TMPDIR:-/tmp}/rawboot-sd.XXXXXX")
        MOUNT=(mount "$SD_PART" "$TMP_MOUNT")
        if [ "${EUID:-$(id -u)}" -ne 0 ]; then
            MOUNT=(sudo "${MOUNT[@]}")
        fi
        "${MOUNT[@]}"
        fstype=$(findmnt -n -o FSTYPE --target "$TMP_MOUNT" 2>/dev/null) ||
            die "cannot determine the filesystem type of $SD_PART"
        case "$fstype" in
            vfat | msdos | fat) ;;
            *) die "$SD_PART is '$fstype', not FAT; use --format or copy payload manually" ;;
        esac
        CP=(cp "$PAYLOAD_IMG" "$TMP_MOUNT/$PAYLOAD_NAME")
        if [ "${EUID:-$(id -u)}" -ne 0 ] && [ ! -w "$TMP_MOUNT" ]; then
            CP=(sudo "${CP[@]}")
        fi
        "${CP[@]}"
        if [ -n "$BOOT_CFG" ]; then
            CP_CFG=(cp "$BOOT_CFG" "$TMP_MOUNT/boot.cfg")
            if [ "${EUID:-$(id -u)}" -ne 0 ] && [ ! -w "$TMP_MOUNT" ]; then
                CP_CFG=(sudo "${CP_CFG[@]}")
            fi
            "${CP_CFG[@]}"
        fi
        sync
        UMOUNT=(umount "$TMP_MOUNT")
        if [ "${EUID:-$(id -u)}" -ne 0 ]; then
            UMOUNT=(sudo "${UMOUNT[@]}")
        fi
        "${UMOUNT[@]}"
        rmdir "$TMP_MOUNT"
        TMP_MOUNT=
    fi
fi

if [ "$IS_BLOCK" = "1" ]; then
    echo "note: start the UART capture before the cold power-on; see docs/agents/hil.md"
    echo "note: never run mkfs.vfat on $SD_DEV; it destroys the loader in sector 16"
fi

if [ "$DRY_RUN" = "1" ]; then
    echo "dry run: nothing was built or written"
elif [ "$IS_BLOCK" = "1" ]; then
    echo "done: $SD_DEV is ready for a cold power-on"
else
    echo "done: $SD_DEV written"
fi
