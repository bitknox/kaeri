//! Profiles, as the picker and the Settings section show them. Optional: a single user has none
//! and the engine never sends this.

/// One profile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    /// One or two letters for the avatar.
    pub initials: String,
    /// A kids profile: the adult filter on and locked.
    pub kids: bool,
    /// The profile in use now.
    pub active: bool,
}

/// All the profiles. Empty when there are none, which is a single user.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProfilesView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub profiles: Vec<ProfileView>,
}
