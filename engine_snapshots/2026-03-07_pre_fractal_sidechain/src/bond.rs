use serde::{Deserialize, Serialize};

/// Whether the bond channel is in the far-field (equilibrium fill)
/// or near-field (shell interaction) regime.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChannelRegime {
    /// The STE filling this channel is at equilibrium — free-flow,
    /// undisturbed.  Attraction propagates through it (gravity-like),
    /// light relays through it, but there is no shell-to-shell
    /// interaction.  The channel is transparent.
    Equilibrium,

    /// The parcels are close enough that their compression shells
    /// overlap.  This is where structure happens: chokes form,
    /// shells lock, light emits, energy exchanges occur.
    ShellInteraction,
}

/// A bond between two STE parcels — the fundamental relationship.
///
/// In this ontology there is no coordinate frame.  There is only
/// "how far apart" and "how fast that's changing."  A bond is the
/// simulation's irreducible unit of spatial relationship.
///
/// The channel between two bonded parcels is **filled with STE** at
/// equilibrium.  It is not empty space.  Attraction and light both
/// propagate through this fill.  The channel only becomes dynamically
/// interesting when the two parcels are close enough for their
/// compression shells to interact (the near-field / ShellInteraction
/// regime).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bond {
    /// Index of the other parcel in this bond.
    pub peer: usize,

    /// Scalar distance between the two parcels.
    /// This is the ONLY spatial quantity.  No direction, no axes.
    pub distance: f64,

    /// Rate of change of distance (negative = approaching, positive = receding).
    pub closing_speed: f64,

    /// Angular offset relative to this parcel's first bond.
    /// Measured in radians.  Preserves chirality (handedness) so we
    /// can detect circulation / vorticity without a coordinate frame.
    ///
    /// For parcel i's bond list:
    ///   bond[0].angle = 0.0 (reference)
    ///   bond[k].angle = angle between bond[0] and bond[k], measured
    ///   around i's local "outward" direction.
    pub angle: f64,

    /// Second angular coordinate (elevation from the reference plane
    /// formed by the parcel's first two bonds).
    /// Together with `angle`, this defines a full solid-angle position
    /// on the parcel's local sphere — without any global axes.
    pub elevation: f64,

    /// Current tension on this bond — the attraction stress.
    /// Positive = under tension (pulling together).
    pub tension: f64,

    /// Which regime the channel is in.
    /// Determined each tick by comparing `distance` to the
    /// shell-interaction range.
    pub regime: ChannelRegime,

    /// Concentration of the STE filling this channel.
    /// At equilibrium this equals the background free-flow density.
    /// Under shell interaction it rises toward the parcels'
    /// compression levels.
    pub channel_concentration: f64,
}

impl Bond {
    /// Create a new bond with equilibrium channel fill.
    pub fn new(peer: usize, distance: f64) -> Self {
        Self {
            peer,
            distance,
            closing_speed: 0.0,
            angle: 0.0,
            elevation: 0.0,
            tension: 0.0,
            regime: ChannelRegime::Equilibrium,
            channel_concentration: 0.0,
        }
    }

    /// Update the channel regime based on current distance.
    pub fn update_regime(&mut self, shell_interaction_range: f64) {
        self.regime = if self.distance <= shell_interaction_range {
            ChannelRegime::ShellInteraction
        } else {
            ChannelRegime::Equilibrium
        };
    }

    /// Whether the parcels' shells are interacting on this bond.
    pub fn is_shell_interacting(&self) -> bool {
        self.regime == ChannelRegime::ShellInteraction
    }
}
