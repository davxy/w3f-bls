use ark_ec::Group;
use ark_ff::PrimeField;

use digest::Digest;

use crate::double::{DoublePublicKeyScheme, PublicKeyInSignatureGroup};
use crate::engine::EngineBLS;
use crate::schnorr_pop::SchnorrProof;
use crate::serialize::SerializableToBytes;
use crate::single::Signature;
use crate::{Message, SecretKeyVT};

pub type ChaumPedersenSignature<E> = (Signature<E>, SchnorrProof<E>);

/// ProofOfPossion trait which should be implemented by secret
pub trait ChaumPedersenSigner<E: EngineBLS, H: Digest> {
    /// The proof of possession generator is supposed to
    /// to produce a schnoor signature of the message using
    /// the secret key which it claim to possess.
    fn generate_cp_signature(&mut self, message: &Message) -> ChaumPedersenSignature<E>;

    fn generate_witness_scaler(
        &self,
        message: &Message,
    ) -> <<E as EngineBLS>::PublicKeyGroup as Group>::ScalarField;

    fn generate_dleq_proof(
        &mut self,
        message: &Message,
        bls_signature: E::SignatureGroup,
    ) -> SchnorrProof<E>;
}

/// This should be implemented by public key
pub trait ChaumPedersenVerifier<E: EngineBLS, H: Digest> {
    fn verify_cp_signature(
        &self,
        message: &Message,
        signature_proof: ChaumPedersenSignature<E>,
    ) -> bool;
}

impl<E: EngineBLS, H: Digest> ChaumPedersenSigner<E, H> for SecretKeyVT<E> {
    fn generate_cp_signature(&mut self, message: &Message) -> ChaumPedersenSignature<E> {
        //First we generate a vanila BLS Signature;
        let bls_signature = SecretKeyVT::sign(self, message);
        (
            bls_signature,
            <SecretKeyVT<E> as ChaumPedersenSigner<E, H>>::generate_dleq_proof(
                self,
                message,
                bls_signature.0,
            ),
        )
    }

    #[allow(non_snake_case)]
    fn generate_dleq_proof(
        &mut self,
        message: &Message,
        bls_signature: E::SignatureGroup,
    ) -> SchnorrProof<E> {
        let mut k =
            <SecretKeyVT<E> as ChaumPedersenSigner<E, H>>::generate_witness_scaler(self, message);

        let signature_point = bls_signature;
        let message_point = message.hash_to_signature_curve::<E>();

        let A_point = <<E as EngineBLS>::SignatureGroup as Group>::generator() * k;
        let B_point = message_point * k;

        let A_point_as_bytes = E::signature_point_to_byte(&A_point);
        let B_point_as_bytes = E::signature_point_to_byte(&B_point);

        let signature_point_as_bytes = E::signature_point_to_byte(&signature_point);
        let message_point_as_bytes = E::signature_point_to_byte(&message_point);
        let public_key_in_signature_group_as_bytes = E::signature_point_to_byte(
            &DoublePublicKeyScheme::<E>::into_public_key_in_signature_group(self).0,
        );

        let random_scalar = <H as Digest>::new()
            .chain_update(message_point_as_bytes)
            .chain_update(public_key_in_signature_group_as_bytes)
            .chain_update(signature_point_as_bytes)
            .chain_update(A_point_as_bytes)
            .chain_update(B_point_as_bytes)
            .finalize();

        let c = <<<E as EngineBLS>::PublicKeyGroup as Group>::ScalarField>::from_be_bytes_mod_order(
            &*random_scalar,
        );
        let s = k - c * self.0;

        ::zeroize::Zeroize::zeroize(&mut k); //clear secret witness from memory

        (c, s)
    }

    fn generate_witness_scaler(
        &self,
        message: &Message,
    ) -> <<E as EngineBLS>::PublicKeyGroup as Group>::ScalarField {
        let secret_key_as_bytes = self.to_bytes();

        let mut scalar_bytes = <H as Digest>::new()
            .chain_update(secret_key_as_bytes)
            .chain_update(message.0)
            .finalize();
        let random_scalar: &mut [u8] = scalar_bytes.as_mut_slice();
        <<<E as EngineBLS>::PublicKeyGroup as Group>::ScalarField>::from_be_bytes_mod_order(
            &*random_scalar,
        )
    }
}

/// This should be implemented by public key
#[allow(non_snake_case)]
impl<E: EngineBLS, H: Digest> ChaumPedersenVerifier<E, H> for PublicKeyInSignatureGroup<E> {
    fn verify_cp_signature(
        &self,
        message: &Message,
        signature_proof: ChaumPedersenSignature<E>,
    ) -> bool {
        let A_check_point = <<E as EngineBLS>::SignatureGroup as Group>::generator()
            * signature_proof.1 .1
            + self.0 * signature_proof.1 .0;

        let B_check_point = message.hash_to_signature_curve::<E>() * signature_proof.1 .1
            + signature_proof.0 .0 * signature_proof.1 .0;

        let A_point_as_bytes = E::signature_point_to_byte(&A_check_point);
        let B_point_as_bytes = E::signature_point_to_byte(&B_check_point);

        let signature_point_as_bytes = signature_proof.0.to_bytes();
        let message_point_as_bytes =
            E::signature_point_to_byte(&message.hash_to_signature_curve::<E>());
        let public_key_in_signature_group_as_bytes = E::signature_point_to_byte(&self.0);

        let resulting_scalar = <H as Digest>::new()
            .chain_update(message_point_as_bytes)
            .chain_update(public_key_in_signature_group_as_bytes)
            .chain_update(signature_point_as_bytes)
            .chain_update(A_point_as_bytes)
            .chain_update(B_point_as_bytes)
            .finalize();
        let c_check =
            <<<E as EngineBLS>::PublicKeyGroup as Group>::ScalarField>::from_be_bytes_mod_order(
                &*resulting_scalar,
            );

        c_check == signature_proof.1 .0
    }
}
