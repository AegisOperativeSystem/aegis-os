#!/usr/bin/env python3
"""Pack an Aegis ISO into a VirtualBox zip with an empty disk."""

import argparse
import os
import struct
import uuid
import zipfile

GIB = 1024 * 1024 * 1024


def vbox_uuid(value: str) -> bytes:
    raw = bytes.fromhex(value.replace("-", ""))
    out = bytearray(16)
    out[0], out[1], out[2], out[3] = raw[3], raw[2], raw[1], raw[0]
    out[4], out[5] = raw[5], raw[4]
    out[6], out[7] = raw[7], raw[6]
    out[8:] = raw[8:]
    return bytes(out)


def create_vdi(size: int, disk_uuid: str) -> bytes:
    block = 1024 * 1024
    blocks = (size + block - 1) // block
    off_blocks = 512
    off_data = off_blocks + blocks * 4
    blob = bytearray(off_data)
    info = b"<<< Oracle VM VirtualBox Disk Image >>>\n"
    blob[: len(info)] = info

    def u32(offset: int, value: int) -> None:
        blob[offset : offset + 4] = struct.pack("<I", value)

    def u64(offset: int, value: int) -> None:
        blob[offset : offset + 8] = struct.pack("<Q", value)

    heads, sectors = 16, 63
    cylinders = size // (heads * sectors * 512)
    u32(64, 0xBEDA107F)
    u32(68, 0x00010001)
    u32(72, 400)
    u32(76, 1)
    u32(340, off_blocks)
    u32(344, off_data)
    u32(348, cylinders)
    u32(352, heads)
    u32(356, sectors)
    u32(360, 512)
    u64(368, size)
    u32(376, block)
    u32(384, blocks)
    u32(388, 0)
    blob[392:408] = vbox_uuid(disk_uuid)
    blob[408:424] = vbox_uuid(disk_uuid)
    u32(456, cylinders)
    u32(460, heads)
    u32(464, sectors)
    u32(468, 512)
    for index in range(blocks):
        u32(off_blocks + index * 4, 0xFFFFFFFF)
    return bytes(blob)


def machine_xml(iso_name: str, disk_uuid: str, iso_uuid: str, machine_uuid: str, firmware: str) -> str:
    safe = (
        iso_name.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )
    return f"""<?xml version="1.0"?>
<VirtualBox xmlns="http://www.virtualbox.org/" version="1.19-windows">
  <Machine uuid="{{{machine_uuid}}}" name="Aegis OS" OSType="ArchLinux_64" snapshotFolder="Snapshots" lastStateChange="2026-09-29T12:00:00Z">
    <MediaRegistry>
      <HardDisks>
        <HardDisk uuid="{{{disk_uuid}}}" location="Aegis OS.vdi" format="VDI" type="Normal"/>
      </HardDisks>
      <DVDImages>
        <Image uuid="{{{iso_uuid}}}" location="{safe}"/>
      </DVDImages>
    </MediaRegistry>
    <Hardware>
      <CPU count="2">
        <HardwareVirtEx enabled="true"/>
        <LongMode enabled="true"/>
      </CPU>
      <Memory RAMSize="2048"/>
      <Firmware type="{firmware}"/>
      <Boot>
        <Order position="1" device="DVD"/>
        <Order position="2" device="HardDisk"/>
        <Order position="3" device="None"/>
        <Order position="4" device="None"/>
      </Boot>
      <Display controller="VMSVGA" VRAMSize="128"/>
      <BIOS>
        <IOAPIC enabled="true"/>
      </BIOS>
      <USB>
        <Controllers>
          <Controller name="OHCI" type="OHCI"/>
        </Controllers>
      </USB>
      <Network>
        <Adapter slot="0" enabled="true" MACAddress="080027A1E615" type="82540EM">
          <NAT/>
        </Adapter>
      </Network>
      <AudioAdapter controller="AC97" useDefault="true" driver="Default" enabled="true"/>
      <RTC localOrUTC="UTC"/>
    </Hardware>
    <StorageControllers>
      <StorageController name="SATA" type="AHCI" PortCount="2" useHostIOCache="false" Bootable="true" IDE0MasterEmulationPort="0" IDE0SlaveEmulationPort="1" IDE1MasterEmulationPort="2" IDE1SlaveEmulationPort="3">
        <AttachedDevice type="HardDisk" hotpluggable="false" port="0" device="0">
          <Image uuid="{{{disk_uuid}}}"/>
        </AttachedDevice>
        <AttachedDevice passthrough="false" type="DVD" hotpluggable="false" port="1" device="0">
          <Image uuid="{{{iso_uuid}}}"/>
        </AttachedDevice>
      </StorageController>
    </StorageControllers>
  </Machine>
</VirtualBox>
"""


def readme(iso_name: str, firmware: str) -> str:
    return f"""Aegis OS VirtualBox machine

This archive already contains {iso_name}.

1. Extract the whole zip into a folder. Do not open the machine from inside the zip.
2. Double-click "Aegis OS.vbox".
3. Start the machine. Firmware is {firmware}. The ISO is already attached.
4. The 16 GiB disk is empty until you install from the live desktop.
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("iso")
    parser.add_argument("-o", "--output", default="Aegis-OS-VirtualBox.zip")
    parser.add_argument("--firmware", choices=("BIOS", "EFI"), default="BIOS")
    args = parser.parse_args()
    iso_name = os.path.basename(args.iso)
    disk_uuid = str(uuid.uuid4())
    iso_uuid = str(uuid.uuid4())
    machine_uuid = str(uuid.uuid4())
    with zipfile.ZipFile(args.output, "w", compression=zipfile.ZIP_STORED) as archive:
        archive.write(args.iso, iso_name)
        archive.writestr("Aegis OS.vdi", create_vdi(16 * GIB, disk_uuid))
        archive.writestr(
            "Aegis OS.vbox",
            machine_xml(iso_name, disk_uuid, iso_uuid, machine_uuid, args.firmware),
        )
        archive.writestr("README.txt", readme(iso_name, args.firmware))


if __name__ == "__main__":
    main()
