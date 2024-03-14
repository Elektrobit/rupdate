Update Bundle (Package)
=======================

An update bundle is an archive that contains one or more partition images and the corresponding metadata needed to update a system using those images. While an update bundle might be used to execute a complete system update, it can also be restricted to only update a certain part of a system like the linux kernel, the rootfs or a specialized configfs.

## Update Bundle Archive

The update bundle archive format is [tar](https://www.gnu.org/software/tar/), a commonly used archiving standard in the unix community. *Optionally* the update bundle can be compressed using [gzip](https://www.gnu.org/software/gzip/), which is also an open source standard widely used in the unix community. Gzip was chosen because of it's streaming capabilities that are a great benefit of using a compression standard build around the [Deflate](https://en.wikipedia.org/wiki/Deflate) algorithm. The only structural requirement to the archive is, that the first file in the archive has to be the update manifest.

## Manifest - The Metadata

The information necessary to verify images and write them to the correct partitions is

### Update Description

| Field            | Description                                                 |
|------------------|-------------------------------------------------------------|
| version          | Manifest version number                                     |
| rollback_allowed | Whether a rollback is allowed after installing this bundle. |
| images           | List of images that are in this bundle                      |

### Image Description

Installing the images included in an update bundle requires to associate images to partitions. This mapping of images to partitions is based on partition set names, which helps separating partitioning details from the update bundle. For this to work the partition set name has to be consistent with the name specified in the [partition configuration](../../partcfgimg/README.md).

| Field            | Description                                                 |
|------------------|-------------------------------------------------------------|
| name             | Name of the partition set this image is meant for.          |
| filename         | Name of the image file in the bundle.                       |
| sha256           | Checksum of the file.                                       |

### Example

```json
{
  "version":"2.0",
  "rollback_allowed": true,
  "images": [
    {
      "name": "bootfs",
      "filename": "bootfs.img",
      "sha256": "10f7e2d04febdfdbc4979fd96c95f59b00ba860e8d335c4bb4d62c6e0520d359",
    },
    {
      "name": "rootfs",
      "filename": "rootfs.img",
      "sha256": "31533a2aad5ebdf2c34fe03746fa2782693415357ee50fc50aab4e58ca6792ce",
    }
  ]
}
```

## How to build bundles

Update bundles that shall be installed by rupdate can be created either from separate images or from combined full images.

### from separate images

If separate images of rootfs, bootfs and others were created by other means, these can be wrapped to bundles using ```update-tool-create-bundle```.

**Example**

```
update-tool-create-bundle.sh bootloader:/<PATH>/bootloader bootfs:/<PATH>/fit.img
```
See ```--help``` of tool for reference documentation.

### from full image

If combined images with rootfs, bootfs and others targeted to be installed on a SD-Card, were created by other means, these can be unwrapped and repackaged to bundles using ```update-tool-img2bundle```.

**Example**

```
update-tool-img2bundle full_linux.img
```
See ```--help``` of tool for reference documentation.
