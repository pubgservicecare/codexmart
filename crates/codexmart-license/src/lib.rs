//! CODE X MART licensing.
//!
//! Shared by the customer application (verification only) and the owner's
//! License Generator (feature `issuer`, which adds signing). Nothing here
//! touches the network: requesting, issuing and activating a license all
//! work on computers that are never online.
//!
//! ```text
//!   customer app                    owner (License Generator)
//!   ------------                    -------------------------
//!   request code  CXMR-...  ----->  decode_request
//!                                   issue_license (Ed25519 private key)
//!   activation key CXMA-... <-----  encode_activation_key
//!   verify_token (public key, this computer's device id)
//! ```
//!
//! Modules:
//! * [`codec`]: code alphabet, grouping and Luhn mod 32 check symbols;
//! * [`device`]: device id from hardware identifiers;
//! * [`license`]: license payload, signatures and activation keys;
//! * [`request`]: request codes;
//! * [`keys`]: public keys trusted by the app;
//! * [`state`]: trial/license state records and the access decision.

pub mod codec;
pub mod device;
pub mod keys;
pub mod license;
pub mod request;
pub mod state;

/// The person customers contact to buy and activate a license.
pub const OWNER_NAME: &str = "Hamad Mansha";
pub const OWNER_PHONE: &str = "+92 348 6825408";
/// WhatsApp chat link for [`OWNER_PHONE`].
pub const OWNER_WHATSAPP_URL: &str = "https://wa.me/923486825408";
