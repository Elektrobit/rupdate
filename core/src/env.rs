// SPDX-License-Identifier: MIT
use crate::{
    fixed_string::FixedString,
    hash_sum::{HashSum, Hashable},
    hex_dump::HexDump,
    partitions::{PartitionConfig, Partitioned},
    state::State,
    variant::Variant,
};
use anyhow::{anyhow, Context, Result};
use bincode::Options;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::{Read, Seek, SeekFrom, Write},
    ops::{Deref, DerefMut},
};

/// Magic number that identifies an update state.
pub static MAGIC: &[u8; 4] = &[b'E', b'B', b'U', b'S'];
/// Number of update state slots
pub const NUM_SLOTS: usize = 2;

/// Positions of update states within the update environment.
#[derive(Copy, Clone)]
#[cfg_attr(debug_assertions, derive(Debug))]
#[repr(usize)]
pub enum EnvironmentSlot {
    First = 0,
    Second = 1,
}

/// Allow conversion from unsigned integer values to update environment slots.
impl TryFrom<usize> for EnvironmentSlot {
    type Error = anyhow::Error;

    fn try_from(value: usize) -> Result<Self> {
        match value {
            value if value == Self::First as usize => Ok(Self::First),
            value if value == Self::Second as usize => Ok(Self::Second),
            _ => Err(anyhow!("Invalid update environment slot {}", value)),
        }
    }
}

/// Selection of partition variants within a partition set.
///
/// A poartition selection consists of the related partition set name,
/// the currently active variant and whether it would be affected by a
/// rollback to an older system or is currently affected by an update.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct PartSelection {
    /// Partition set name (36 byte ascii string)
    pub set_name: FixedString<36>,
    /// Active variant (char 'a' or 'b')
    pub active: Variant,
    // Whether or not a rollback possible and allowed.
    pub rollback: bool,
    // Whether or not this set has been affected by the latest update.
    pub affected: bool,
}

/// Implement display trait for the update environment as hex dump.
impl fmt::Display for PartSelection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#?}", bincode::serialize(&self))
    }
}

/// Data of an update state.
///
/// This struct is manly used for separating the actual
/// contents of an update state from the hash sum in order
/// to ease hash calculations.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct UpdateStateData {
    /// A magic value identifying an environment
    pub magic: [u8; 4],
    /// 4 byte version number
    pub version: u32,
    /// Number of updates done, used to determine the most recent env from the multiple versions.
    pub env_revision: u32,
    /// Number of remaining boot attempts of the active partition.
    /// -1 permanently selected, 0 no tries left and n tries left otherwise.
    pub remaining_tries: i16,
    /// Current system state
    pub state: State,
    /// Array of `partsel_count` partition selections
    pub partition_selection: Vec<PartSelection>,
}

/// Default values for a new update state
impl Default for UpdateStateData {
    fn default() -> Self {
        Self {
            magic: MAGIC.to_owned(),
            version: 0x00000001,
            env_revision: 0x00,
            remaining_tries: -1,
            partition_selection: Vec::new(),
            state: State::Normal,
        }
    }
}

/// Simplifies hashing of update state data
impl Hashable for UpdateStateData {
    /// Returns the bincode binary representation of an update state data
    fn raw(&self) -> Result<Vec<u8>> {
        Ok(bincode::options().with_fixint_encoding().serialize(&self)?)
    }
}

/// Content of an update environment slot.
///
/// The update environment consists of two slots, the active one and
/// an older or newer installation based on the current update state.
/// Each of these slots consisting of a magic number, a version,
/// the partition selection and a crc over the former fields.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct UpdateState {
    /// State data
    pub data: UpdateStateData,
    /// Hash sum
    pub hash_sum: HashSum,
}

/// Allow transparent access to the internal data of an update state
impl Deref for UpdateState {
    type Target = UpdateStateData;
    #[inline]
    fn deref(&self) -> &UpdateStateData {
        &self.data
    }
}

/// Allow  mutable access to the internal data of an update state
impl DerefMut for UpdateState {
    #[inline]
    fn deref_mut(&mut self) -> &mut UpdateStateData {
        &mut self.data
    }
}

impl HexDump for UpdateState {}

/// Implement display trait for the update state as hex dump.
impl fmt::Display for UpdateState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.hex_dump(f)
            .context("Failed to serialize update state.")
            .map_err(|_| fmt::Error)
    }
}

/// Simplifies hashing of an update state
impl Hashable for UpdateState {
    /// Returns the bincode binary representation of an update state
    fn raw(&self) -> Result<Vec<u8>> {
        Ok(bincode::options().with_fixint_encoding().serialize(&self)?)
    }
}

impl UpdateState {
    /// Returns a new instance of the UpdateState.
    ///
    /// Initializes the update state based on the given configuration.
    ///
    /// # Error
    ///
    /// Returns an error if reading of update state failed.
    pub fn new(part_config: &PartitionConfig) -> Result<Self> {
        let mut new_state = Self {
            data: UpdateStateData::default(),
            hash_sum: HashSum::from(part_config.hash_algorithm.clone()),
        };

        for set in part_config
            .partition_sets
            .iter()
            .filter(|set| set.partitions.len() == 2)
        {
            new_state.partition_selection.push(PartSelection {
                set_name: set.name.parse()?,
                ..PartSelection::default()
            })
        }

        new_state
            .update_hash_sum()
            .context("Failed to update state hashsum.")?;

        Ok(new_state)
    }

    /// Returns a new instance of the update state.
    ///
    /// Initializes the update state based on the given partition configuration
    /// and device handler and reads the update state, placed in raw memory in front of the
    /// bootloader.
    ///
    /// # Error
    ///
    /// Returns an error if reading of update state failed.
    pub fn from_memory<T>(dp: T) -> Result<Self>
    where
        T: Read + Write + Seek,
    {
        bincode::options()
            .with_fixint_encoding()
            .deserialize_from::<T, Self>(dp)
            .context("Deserialization of update state failed.")
    }

    /// Clean the current state and partition selection.
    ///
    /// Sets the current state to normal and clears the affected and rollback
    /// flags for all partition selections, finally resetting the remaining
    /// try counter.
    pub fn clean(&mut self, allow_rollback: bool) {
        self.state = State::Normal;

        for partsel in &mut self.partition_selection {
            partsel.affected = false;
            partsel.rollback &= allow_rollback;
        }

        self.remaining_tries = -1;
    }

    /// Disables the rollback for all partition selections.
    pub fn disable_rollback(&mut self) {
        for partsel in self.partition_selection.iter_mut() {
            partsel.rollback = false;
        }
    }

    /// Returns the hash sum over the raw encoded update state data.
    ///
    /// # Error
    ///
    /// Returns an error if generating the update state hash failed.
    pub fn hash_sum(&self) -> Result<HashSum> {
        let serialized = self.data.raw()?;
        HashSum::generate(serialized.as_slice(), self.hash_sum.algorithm())
    }

    /// Updates the hash sum over the raw encoded update state data.
    ///
    /// # Error
    ///
    /// Returns an error if generating the update state hash failed.
    pub fn update_hash_sum(&mut self) -> Result<()> {
        let serialized = self.data.raw()?;
        self.hash_sum = HashSum::generate(serialized.as_slice(), self.hash_sum.algorithm())?;

        Ok(())
    }

    /// Verify an update state.
    ///
    /// Verifies the magic number and the crc of an update state.
    ///
    /// # Error
    ///
    /// If the magic or the crc is invalid an error will be returned.
    pub fn verify(&self) -> Result<()> {
        if self.magic.as_slice() != MAGIC {
            return Err(anyhow!("Magic verification of update update state failed."));
        }

        if self.hash_sum != self.hash_sum()? {
            return Err(anyhow!(
                "Hash sum verification of update update state failed."
            ));
        }

        Ok(())
    }

    /// Returns whether an update state is valid.
    ///
    /// Returns true if the magic number and the crc of an update state
    /// are correct, false otherwise.
    pub fn is_valid(&self) -> bool {
        if let Ok(hash_sum) = self.hash_sum() {
            self.magic.as_slice() == MAGIC && self.hash_sum == hash_sum
        } else {
            false
        }
    }

    /// Marks the partition of the given partition set as been updated.
    ///
    /// # Error
    ///
    /// Returns an error if no partition selection could be found.
    pub fn mark_new(&mut self, set_name: &str) -> Result<()> {
        self.partition_selection
            .iter_mut()
            .find(|partsel| partsel.set_name == set_name)
            .with_context(|| {
                format!(
                    "Failed to find partition selection for {set_name} in current update state."
                )
            })?
            .affected = true;

        Ok(())
    }

    /// Allow rollback of the given partition selection.
    ///
    /// # Error
    ///
    /// Returns an error if no partition selection could be found.
    pub fn allow_rollback(&mut self, set_name: &str) -> Result<()> {
        self.partition_selection
            .iter_mut()
            .find(|partsel| partsel.set_name == set_name)
            .with_context(|| {
                format!(
                    "Failed to find partition selection for {set_name} in current update state."
                )
            })?
            .rollback = true;

        Ok(())
    }

    /// Return the partition selection.
    ///
    /// Returns 0 if partition A is selected within the given
    /// partition set and 1 if B is selected.
    ///
    /// # Error
    ///
    /// Returns an error if no partition selection could be found.
    pub fn get_selection(&self, partition_set: &str) -> Result<Variant> {
        self.partition_selection
            .iter()
            .find_map(|part| {
                if part.set_name == partition_set {
                    Some(part.active)
                } else {
                    None
                }
            })
            .with_context(|| format!("Failed to find partition selection for {partition_set} in current update state."))
    }
}

/// The update environment.
///
/// The update environment is used for sharing a common state between
/// the linux system and the bootloader. Thus the update can maintain state
/// between reboots, while the bootloader can examine which partitions to mount
/// and which kernel + dtb to boot.
///
/// The update environment consists of two update states, which hold the partition
/// configuration for the currently active and an older system.
///
/// As the update environment is placed in raw memory in front of the bootloader,
/// the environment also needs information about the offset of itself in memory and the
/// spacing of the update states. This information is provided by the partition configuration.
///
/// The environment is accessed through a handler interface passed in during construction.
///
/// # Example
///
/// ```no_run
/// use rupdate_core::{
///     partitions::PartitionConfig,
///     env::Environment,
/// };
/// use std::fs::File;
///
/// let part_config = PartitionConfig::new("partitions.json").unwrap();
/// let dp = File::open("/dev/mmcblkX").unwrap();
/// let env = Environment::new(&part_config, dp).unwrap();
/// ```
pub struct Environment<'a, T>
where
    T: Read + Write + Seek,
{
    /// Pointer to the environment device
    dp: T,
    /// Reference to update tool configuration
    part_config: &'a PartitionConfig,
    /// Environment states
    update_states: [UpdateState; NUM_SLOTS],
}

/// Allows to dump the update environment using a simple println!().
impl<'a, T> fmt::Display for Environment<'a, T>
where
    T: Read + Write + Seek,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, state) in self.update_states.iter().enumerate() {
            writeln!(f, "Update State {i}:")?;
            writeln!(f, "{state}")?;
        }

        Ok(())
    }
}

impl<'a, T> Environment<'a, T>
where
    T: Read + Write + Seek,
{
    /// Returns a new instance of the Environment.
    ///
    /// Initializes the environment based on the given configuration.
    ///
    /// # Error
    ///
    /// Returns an error if reading of update environment failed.
    pub fn new(part_config: &'a PartitionConfig, dp: T) -> Result<Self> {
        // Ensure an update environment is configured.
        part_config
            .find_update_part()
            .context("Failed to find update environment partition.")?;

        let new_states = [(); NUM_SLOTS]
            .iter()
            .map(|_| UpdateState::new(part_config))
            .collect::<Result<Vec<UpdateState>>>()?;

        let new_states: Box<[UpdateState; NUM_SLOTS]> =
            match new_states.into_boxed_slice().try_into() {
                Ok(v) => v,
                Err(_) => unreachable!(),
            };

        Ok(Self {
            dp,
            part_config,
            update_states: *new_states,
        })
    }

    /// Initializes an instance of the Environment from the given reader.
    ///
    /// Initializes the environment based on the given configuration
    /// and device handler and reads the update states from the
    /// update environment, placed in raw memory in front of the
    /// bootloader.
    ///
    /// # Error
    ///
    /// Returns an error if reading of update environment failed.
    pub fn from_memory(part_config: &'a PartitionConfig, dp: T) -> Result<Self> {
        // Ensure an update environment is configured.
        part_config
            .find_update_part()
            .context("Failed to find update environment partition.")?;

        let mut env = Self {
            dp,
            part_config,
            update_states: Default::default(),
        };
        env.read()?;

        Ok(env)
    }

    /// Seek to the given update state.
    ///
    /// Seeks to the environment offset + the update state offset.
    ///
    /// # Error
    ///
    /// Returns an error in case of failure.
    fn seek_state(&mut self, index: usize) -> Result<()> {
        let update_part_set = self
            .part_config
            .find_update_fs()
            .context("Could not find update environment in partition config.")?;

        let linux_part = self
            .part_config
            .find_update_part()
            .context("Could not find update environment partition in partition config.")?;

        let state_offset = match update_part_set.user_data.get("blob_offset") {
            Some(val) => {
                if val.starts_with("0x") {
                    let val = val.trim_start_matches("0x");
                    u64::from_str_radix(val, 16).context("Invalid update state offset.")?
                } else {
                    val.parse::<u64>().context("Invalid update state offset.")?
                }
            }
            None => 0x00,
        };

        if let Partitioned::RawPartition { device: _, offset } = linux_part {
            let state_offset = offset + (index as u64) * state_offset;
            self.dp.seek(SeekFrom::Start(state_offset))?;

            Ok(())
        } else {
            Err(anyhow!("Update environment partition type has to be raw."))
        }
    }

    /// Read the update state.
    ///
    /// # Error
    ///
    /// If reading of the update environment fails, an error is returned.
    fn read_state(&mut self, state: usize) -> Result<UpdateState> {
        self.seek_state(state)?;

        bincode::options()
            .with_fixint_encoding()
            .deserialize_from(&mut self.dp)
            .with_context(|| format!("Reading update state {state} failed."))
    }

    /// Read all states of the update environment.
    ///
    /// # Error
    ///
    /// If reading of the update environment fails, an error is returned.
    fn read(&mut self) -> Result<()> {
        self.update_states = Default::default();

        for i in 0..NUM_SLOTS {
            self.update_states[i] = self
                .read_state(i)
                .with_context(|| format!("Failed to read state {i} of update environment"))?;
        }

        Ok(())
    }

    /// Writes the specified update state.
    ///
    /// Writes the given update state to the specified update state.
    ///
    /// # Error
    ///
    /// If writing of the update state fails, an error is returned.
    pub fn write_state(&mut self, state: &mut UpdateState, slot: EnvironmentSlot) -> Result<()> {
        self.seek_state(slot as usize)?;

        state
            .update_hash_sum()
            .context("Failed to update state hash.")?;

        self.dp
            .write_all(&state.raw().context("Serializing update state failed.")?)?;

        self.update_states[slot as usize] = state.clone();

        Ok(())
    }

    /// Write the given state to the next slot.
    ///
    /// Core function of the update process, as it writes the given state to the
    /// next slot, which is the one not used by the current state.
    ///
    /// # Error
    ///
    /// If writing of the update state fails, an error is returned.
    pub fn write_next_state(&mut self, state: &mut UpdateState) -> Result<()> {
        let next_slot = self
            .next_state_slot()
            .context("Failed to detect next update state slot.")?;

        // The latest state is identified by the highest environment revision.
        state.env_revision += 1;

        self.write_state(state, next_slot)
    }

    /// Write all states of the update environment.
    ///
    /// # Error
    ///
    /// If writing of the update environment fails, an error is returned.
    pub fn write(&mut self) -> Result<()> {
        for slot in 0..NUM_SLOTS {
            self.seek_state(slot)?;

            self.update_states[slot]
                .update_hash_sum()
                .context("Failed to update state hash.")?;

            self.dp.write_all(
                &self.update_states[slot]
                    .raw()
                    .context("Serializing update state failed.")?,
            )?;
        }

        Ok(())
    }

    /// Returns a reference to the specified update state.
    pub fn update_state(&self, state: EnvironmentSlot) -> &UpdateState {
        &self.update_states[state as usize]
    }

    /// Clears the specified update state.
    ///
    /// The specified update state is cleared by writing an empty
    /// update state to it.
    ///
    /// # Error
    ///
    /// If writing of the update environment fails, an error variant is returned.
    pub fn clear_state(&mut self, state: EnvironmentSlot) -> Result<()> {
        let mut default_state = UpdateState::default();
        self.write_state(&mut default_state, state)
    }

    /// Copy one state into another one.
    ///
    /// Copies the update state of one update state into another one.
    pub fn copy_state(&mut self, from: EnvironmentSlot, to: EnvironmentSlot) -> Result<()> {
        let mut new_val = self.update_states[from as usize].clone();
        self.write_state(&mut new_val, to)
    }

    /// Returns the current state.
    ///
    /// The current state represents the current state
    /// of the system, which might not be the same as the booted state.
    pub fn get_current_state(&self) -> Result<&UpdateState> {
        let state1 = self.update_state(EnvironmentSlot::First);
        let state2 = self.update_state(EnvironmentSlot::Second);

        Ok(match (state1.is_valid(), state2.is_valid()) {
            (true, true) => {
                if state1.env_revision >= state2.env_revision {
                    state1
                } else {
                    state2
                }
            }
            (true, false) => state1,
            (false, true) => state2,
            _ => return Err(anyhow!("Failed to detect valid update state.")),
        })
    }

    /// Returns the slot for the next state.
    ///
    /// The next state slot is the slot in which a new state should be written to.
    pub fn next_state_slot(&self) -> Result<EnvironmentSlot> {
        let current_state = self.get_current_state()?;

        let state1 = self.update_state(EnvironmentSlot::First);

        Ok(if state1 == current_state {
            EnvironmentSlot::Second
        } else {
            EnvironmentSlot::First
        })
    }
}

#[cfg(test)]
mod test {
    use super::{Environment, NUM_SLOTS};
    use crate::{
        env::UpdateState,
        partitions::{
            Partition, PartitionConfig, PartitionSet, Partitioned, UPDATE_ENV_FILESYSTEM,
            UPDATE_ENV_SET,
        },
    };
    use mockall::{mock, predicate};
    use std::io::{Error, Read, Seek, SeekFrom, Write};
    use std::result;

    pub type Result<T> = result::Result<T, Error>;

    mock! {
        // Only mock the required methods
        File {}

        impl Seek for File {
            fn seek(&mut self, pos: SeekFrom) -> Result<u64>;
        }

        impl Read for File {
            fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
            fn read_exact(&mut self, buf: &mut [u8]) -> Result<()>;
        }

        impl Write for File {
            fn write(&mut self, buf: &[u8]) -> Result<usize>;
            fn write_all(&mut self, buf: &[u8]) -> Result<()>;
            fn flush(&mut self) -> Result<()>;
        }
    }

    fn mock_read_states(_part_config: &PartitionConfig, file_mock: &mut MockFile) {
        for state_index in 0..NUM_SLOTS {
            let expected_offset = 0x200000 + state_index as u64 * 0x1000;

            file_mock
                .expect_seek()
                .with(predicate::eq(SeekFrom::Start(expected_offset)))
                .times(1)
                .returning(move |_| Ok(expected_offset));
        }

        file_mock.expect_read_exact().returning(|_| Ok(()));
    }

    fn default_part_config() -> PartitionConfig {
        PartitionConfig {
            partition_sets: vec![PartitionSet {
                name: UPDATE_ENV_SET.to_string(),
                filesystem: Some(UPDATE_ENV_FILESYSTEM.to_string()),
                user_data: vec![("blob_offset".to_string(), "0x1000".to_string())]
                    .into_iter()
                    .collect(),
                partitions: vec![Partition {
                    variant: None,
                    linux: Some(Partitioned::RawPartition {
                        device: "mmcblk0".to_string(),
                        offset: 0x200000,
                    }),
                    ..Partition::default()
                }],
                ..PartitionSet::default()
            }],
            ..PartitionConfig::default()
        }
    }

    #[test]
    fn test_new_env() {
        let part_config = default_part_config();
        let file_mock = MockFile::new();

        let env = Environment::<MockFile>::new(&part_config, file_mock);

        assert!(env.is_ok());

        let env = env.unwrap();

        assert_eq!(env.part_config, &part_config);
    }

    #[test]
    fn test_load_env() {
        let part_config = default_part_config();

        let mut file_mock = MockFile::new();
        mock_read_states(&part_config, &mut file_mock);

        let env = Environment::<MockFile>::from_memory(&part_config, file_mock);

        assert!(env.is_ok());

        let env = env.unwrap();

        assert_eq!(env.part_config, &part_config);
    }

    #[test]
    fn test_seek_state_success() {
        let part_config = default_part_config();

        for state_index in 0..3usize {
            let expected_offset = 0x200000 + state_index as u64 * 0x1000;

            let mut file_mock = MockFile::new();
            file_mock
                .expect_seek()
                .with(predicate::eq(SeekFrom::Start(expected_offset)))
                .times(1)
                .returning(move |_| Ok(expected_offset));

            let mut env = Environment::<MockFile> {
                part_config: &part_config,
                dp: file_mock,
                update_states: Default::default(),
            };

            assert!(env.seek_state(state_index).is_ok());
        }
    }

    #[test]
    fn test_read_state() {
        let part_config = default_part_config();

        for state_index in 0..NUM_SLOTS {
            let expected_offset = 0x200000 + state_index as u64 * 0x1000;

            let mut file_mock = MockFile::new();
            file_mock
                .expect_seek()
                .with(predicate::eq(SeekFrom::Start(expected_offset)))
                .times(1)
                .returning(move |_| Ok(expected_offset));

            file_mock.expect_read_exact().returning(|_| Ok(()));

            let mut env = Environment::<MockFile> {
                part_config: &part_config,
                dp: file_mock,
                update_states: Default::default(),
            };

            assert!(env.read_state(state_index).is_ok());
        }
    }

    #[test]
    fn test_write_state() {
        let part_config = default_part_config();

        for state_index in 0..NUM_SLOTS {
            let expected_offset = 0x200000 + state_index as u64 * 0x1000;

            let mut file_mock = MockFile::new();
            file_mock
                .expect_seek()
                .with(predicate::eq(SeekFrom::Start(expected_offset)))
                .times(1)
                .returning(move |_| Ok(expected_offset));

            file_mock.expect_write_all().times(1).returning(|_| Ok(()));

            let mut env = Environment::<MockFile> {
                part_config: &part_config,
                dp: file_mock,
                update_states: Default::default(),
            };

            let mut update_state = UpdateState::default();

            assert!(env
                .write_state(&mut update_state, state_index.try_into().unwrap())
                .is_ok());
        }
    }

    #[test]
    fn test_read_states() {
        let part_config = default_part_config();

        let mut file_mock = MockFile::new();
        mock_read_states(&part_config, &mut file_mock);

        let mut env = Environment::<MockFile> {
            part_config: &part_config,
            dp: file_mock,
            update_states: Default::default(),
        };

        assert!(env.read().is_ok());
    }
}
