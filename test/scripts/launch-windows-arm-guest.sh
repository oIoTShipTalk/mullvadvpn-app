#!/usr/bin/env bash

set -eux

swtpm socket -t  --ctrl type=unixio,path=".tpm/tpmsock"  --tpmstate dir=.tpm/ --tpm2 -d

qemu-system-aarch64 \
  -M virt -m 8G -cpu host -smp 8 \
  -bios ./QEMU_EFI.fd \
  -accel hvf \
  -device ramfb \
  -device qemu-xhci -device usb-kbd -device usb-tablet \
  -nic user,model=virtio-net-pci \
  -drive file=./win11-arm64.qcow2 \
  -drive if=none,id=virtio-drivers,format=raw,media=cdrom,file=./virtio-win.iso \
  -device usb-storage,drive=virtio-drivers \
  -serial stdio \
  -drive if=none,id=install,format=raw,media=cdrom,file=./Win11.iso \
  -device usb-storage,drive=install \
  -tpmdev emulator,id=tpm0,chardev=chrtpm -chardev socket,id=chrtpm,path=".tpm/tpmsock" -device tpm-tis-device,tpmdev=tpm0
