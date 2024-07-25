# Details of rupdate update flow
More details and especially the swing between A and be is shown in the following

![](doc/images/upd_flow.svg)

<details>
  <summary>PlantUML Code</summary>

```plantuml
@startuml upd_flow


[*] --> A_inuse
A_inuse: always boots to same version, not updating
A_inuse: A is in use

A_inuse --> B_installed: install upd-bundle to B
B_installed: a new version was installed in B

B_installed --> B_try: reboot
B_try: Try if B is passing selftest
B_try: Prepare revert to A if B is failing or unresponsive

B_try --> B_inuse: finish update
B_inuse: Always use B

B_try --> A_inuse: not finish

B_inuse --> A_installed: install upd-bundle to A
A_installed:  a new version was installed in B

A_installed --> A_try: reboot
A_try: Try if A is passing selftest
A_try: Prepare revert to B if A is failing or unresponsive

A_try --> B_inuse: not finish

A_try --> A_inuse: finish update

A_inuse --> B_inuse: rollback (if possible)
B_inuse --> A_inuse: rollback (if possible)

@enduml
```
</details>

**Explanations:**

* *install*: A bundle is installed using Rupdate
* *finish*: An installed update was successfully selftested and shall now be used permanently
* *rollback*: A version already in full use shall be discarded and the former one shall be used (if permitted)

These functionalities are implemented in [Rupdate](rupdate/manual.txt)

* not finish / revert: A new version failed in selftest or did run out of tries, hence it need to be reverted, see [bootloader](./bootloader/)

# Install of an update

The installation of an update bundle is initiated by a higher level application, that hands the bundle over to ```rupdate``` for installation.
It is assumed that this app verifies the origin, integrity and compatibility of the update.

- **integrity**: verify that the bundle was not modified
- **origin**: verify that the bundle was created by authorized parties
- **compatibility**: verify that the bundle is intended to be installed on this device (e.g. correct hardware, etc.)

It is assumed that the  app decrypts the bundle in order to maintain **confidentiality**, if needed.

![](doc/images/upd_bundle_inst_flow.svg)


<details>
  <summary>PlantUML Code</summary>

```plantuml
@startuml upd_bundle_inst_flow

hide footbox
autonumber

actor "Integrator" as sysint
box "Build Environment"
        participant "update-tool-create-bundle" as sdk
end box

        participant "some\napp" as capp

box "System"
        participant "Rupdate" as updtool
        database "update environment" as updenv
        database "partition config" as partcfg
        database "partitions" as parts
end box

mainframe uc install of update bundle


== Update installation ==


sdk -> sysint : make bundle available

sysint -> capp : stream the update bundle

capp -> capp : streaming verification of integrity \n and compatibility

capp -> updtool : stream the update bundle

updenv -> updtool : get the current version state

partcfg <- updtool : get the partition configuration of device

updenv <- updtool : prohibit/allow rollback

updtool -> parts : write images as configured

capp -> updtool : terminate stream \n e.g. on error

alt stream termination without error

  updtool -> updenv :  set flags and state

  updtool -> capp : report success

else stream termination with error
  updtool -> capp : report error
end

@enduml
```
</details>

The structure of update environment and partition config is documented here:

* [update environment](updenvimg/README.md)
* [partition config](partcfgimg/README.md)


### How to install an update

Call ``` rupdate update -b <bundle-file>``` to install an update-bundle.


# Bootup

During bootup bootloaders select which version of the OS to boot. Following diagrams shows such a flow:

![](doc/images/upd_bundle_norm_boot_flow.svg)


<details>
  <summary>PlantUML Code</summary>

```plantuml
@startuml upd_bundle_norm_boot_flow

hide footbox
autonumber


box "System"
        participant "hardware" as hw
        participant "bootloader" as bootldr
        database "update environment" as updenv
        database "partition config" as partcfg
        database "partitions" as parts
        participant "OS" as os
end box

mainframe boot of software version


== Bootloader ==

hw -> bootldr : pass control

updenv <- bootldr : read current update state

alt normal boot, no updates done

partcfg -> bootldr : read partition configurations
bootldr -> bootldr :  select partitions to be used
bootldr <-  parts :  load or use data from current partition

else  boot into a newly installed version

partcfg -> bootldr : read partition configurations
bootldr -> bootldr : select partitions to be used
bootldr -> updenv : countdown of tries
bootldr <-  parts :  load or use data from new partition

else no more tries of new version left

partcfg -> bootldr : read partition configurations
bootldr -> bootldr : use former version
bootldr -> updenv : configure former version as recent
bootldr <-  parts :  load or use data from old partition

end


bootldr -> os : pass control



@enduml
```
</details>

### How to boot

To correctly boot a system have these [patches](bootloader/) added to your bootloader.

Or implement feature as described in this [requirements](bootloader/requirements_to_bootloader.md).


## Update finish

After boot of a new version a self test shall be performed to ensure correct functionality.

The selftest needs to be committed by some app in user space. The definition of the selftest and especially its scope is left to the integrator. The integrator has to decide if a selftest after an update is pass or fail.

Examples:

- are all needed nodes still reachable on the network
- is a communication still possible or are any protocol mismatches detected

It is left to the integrator to implement any data migration after successful selftest.

Examples:

- convert end user (driver) configuration data to new data format
- convert and recalculate any calibration data specific to the exemplar.



![](doc/images/upd_bundle_finish_flow.svg)


<details>
  <summary>PlantUML Code</summary>

```plantuml
@startuml upd_bundle_finish_flow

hide footbox
autonumber


box "System"
        participant "hardware" as hw
        participant "bootloader" as bootldr
        database "update environment" as updenv
        participant "Kernel" as os
        participant "Rupdate" as updtool
        database "update environment" as updenv
end box

box "Userland"
        participant "app" as capp
        participant "selftest\n & migration" as selftest
end box

mainframe uc finish of new software version


== Start new version ==

hw -> bootldr : pass control

alt new version is able to boot

  bootldr -> os : pass control
  os -> capp : pass control
  capp -> updtool : query update state

  alt state == testing

  capp -> selftest : pass control
  selftest -> selftest: do selftest and migrate data

  alt selftest pass

    selftest -> updtool : finish the update
    updtool -> updenv : make setting persistent\nset remaining_tries=-1
    selftest -> capp : normal operation

  else selftest fail

    selftest -> os : reboot to revert the new version and fallback to prev.

  end

  else other state
    capp -> capp: normal operation
  end

else new version is unable to boot

  bootldr -> bootldr: reboot and fallback if tries are exhausted

end


@enduml
```
</details>

### How to finish an update

Call ``` rupdate finish``` to finish an update after successful selftest.


## Generation of update bundles

Update bundles contain a new version of the software, the structure of bundles is described in detail [here](scripts/bundle/README.md).


The generator for update bundles can be seen as part of the SDK and is executed in the same environment.

## Generation of images

The images containing the important information to allow Rupdate and bootloader to operate correctly are created as described:

```partcfgimg``` Tool to generate partition configurations, used during deployment, see [partcfgimg](./partcfgimg/) 

```updenvimg``` Tool to generate update environment images, used during deployment, see
[update-tool-create-bundle](./scripts/bundle)
