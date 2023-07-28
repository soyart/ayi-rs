use crate::errors::AyiError;
use crate::linux;
use crate::manifest;
use crate::utils::fs::file_exists;

pub fn do_disks(disks: &[manifest::ManifestDisk]) -> Result<(), AyiError> {
    for disk in disks.iter() {
        if !file_exists(&disk.device) {
            return Err(AyiError::NoSuchDevice(disk.device.to_string()));
        }
    }

    Ok(())
}

fn do_disk(disk: &manifest::ManifestDisk) -> Result<(), AyiError> {
    let create_table_cmd = linux::fdisk::create_table_cmd(&disk.device, &disk.table);
    linux::fdisk::run_fdisk_cmd(&disk.device, &create_table_cmd)?;

    for (n, part) in disk.partitions.iter().enumerate() {
        let create_part_cmd = linux::fdisk::create_partition_cmd(&disk.table, n + 1, part);

        linux::fdisk::run_fdisk_cmd(&disk.device, &create_part_cmd)?;
    }

    Ok(())
}
