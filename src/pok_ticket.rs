use std::io::Cursor;

use ff_zeroize::Field;
use pairing_plus::{bls12_381::{G1, Fr}, CurveProjective, serdes::SerDes};

use crate::{prelude::*, rand_non_zero_fr, TicketProofChallenge};


#[allow(dead_code)]
/// PoK of a ticket and a valid signature
pub struct PoKOfTicketProof {
    /// A' in section 4.5
    pub(crate) a_prime: G1,
    /// \overline{A} in section 4.5
    pub(crate) a_bar: G1,
    /// d in section 4.5
    pub(crate) d: G1,
    /// C in my paper
    pub(crate) c: G1,
    /// Proof of relation a_bar / d == a_prime^{-e} * h_0^r2
    pub(crate) proof_vc_1: ProofG1,
    /// Proof of relation g1 * h1^m1 * h2^m2.... for all disclosed messages m_i == d^r3 * h_0^{-s_prime} * h1^-m1 * h2^-m2.... for all undisclosed messages m_i
    pub(crate) proof_vc_2: ProofG1,
    // proof of relation C = g1^rho1*g2^rho2
    pub(crate) proof_vc_3: ProofG1,
    // proof of relation 1 = C^{-e}g1^{beta1}g2^{beta2}
    pub(crate) proof_vc_4: ProofG1,
    // proof of relation 1 = b^{beta1}t^{-rho1}
    pub(crate) proof_vc_5: ProofG1,
}


/// XXX: An optimization would be to combine the 2 relations into one by using the same techniques as Bulletproofs
#[derive(Debug, Clone)]
pub struct PoKOfTicket {
    /// Building on PoKOfSignature
    pok_sig : PoKOfSignature,
    /// C in my paper
    c : G1,
    // For proving relation C = g1^rho1*g2^rho2
    pok_vc_3: ProverCommittedG1,
    /// The blinding factors
    secrets_3: Vec<Fr>,
    // For proving relation 1 = C^{-e}g1^{beta1}g2^{beta2}
    pok_vc_4: ProverCommittedG1,
    /// The blinding factors
    secrets_4: Vec<Fr>,
    // For proving relation 1 = b^{beta1}t^{-rho1}
    pok_vc_5: ProverCommittedG1,
    /// The blinding factors
    secrets_5: Vec<Fr>,
}


impl PoKOfTicket {
    /// Creates the initial proof data before a Fiat-Shamir calculation
    pub fn init(
        signature: &Signature,
        pok_sig : PoKOfSignature,
        t: G1,
        b: G1,
    ) -> Result<Self, BBSError> {

        let rho1 = rand_non_zero_fr();
        let rho2 = rand_non_zero_fr();

        let mut beta1 = rho1;
        beta1.add_assign(&signature.e);

        let mut beta2 = rho2;
        beta2.add_assign(&signature.e);

        let mut c = G1::one();
        c.mul_assign(rho1);
        let mut g2 = get_g2();
        g2.mul_assign(rho2);
        c.add_assign(&g2);


        // For proving relation C = g1^rho1*g2^rho2
        let mut committing_3 = ProverCommittingG1::new();
        let mut secrets_3 = Vec::with_capacity(2);
        // For g1^{rho1}
        committing_3.commit(&GeneratorG1(G1::one()));
        secrets_3.push(rho1);
        // For g2^{rho2}
        committing_3.commit(&GeneratorG1(get_g2()));
        secrets_3.push(rho2);
        let pok_vc_3 = committing_3.finish();


        // For proving relation 1 = C^{-e}g1^{beta1}g2^{beta2}
        let mut committing_4 = ProverCommittingG1::new();
        let mut secrets_4 = Vec::with_capacity(3);
        // For C^{-e}
        committing_4.commit(&GeneratorG1(c));
        let mut sig_e = signature.e;
        sig_e.negate();
        secrets_4.push(sig_e);
        // For g1^{beta1}
        committing_4.commit(&GeneratorG1(G1::one()));
        secrets_4.push(beta1);
        // For g2^{beta2}
        committing_4.commit(&GeneratorG1(get_g2()));
        secrets_4.push(beta2);
        let pok_vc_4 = committing_4.finish();


        // For proving relation 1 = b^{beta1}t^{-rho1}
        let mut committing_5 = ProverCommittingG1::new();
        let mut secrets_5 = Vec::with_capacity(2);
        // For b^{beta1} (b=hash(...))
        committing_5.commit(&GeneratorG1(b));
        secrets_5.push(beta1);
        // For t^{-rho1}
        committing_5.commit(&GeneratorG1(t));
        let mut neg_rho1 = rho1;
        neg_rho1.negate();
        secrets_5.push(neg_rho1);
        let pok_vc_5 = committing_5.finish();


        Ok(PoKOfTicket {
            pok_sig,
            c,
            pok_vc_3,
            secrets_3,
            pok_vc_4,
            secrets_4,
            pok_vc_5,
            secrets_5,
        })
    }


    /// Return byte representation of public elements so they can be used for challenge computation.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];

        // For signature PoK values
        self.pok_sig.to_bytes().append(&mut bytes);

        // For 3rd PoKVC
        bytes.append(&mut self.pok_vc_3.to_bytes());

        // For 4th PoKVC
        // self.C is included as part of self.pok_vc_4
        bytes.append(&mut self.pok_vc_4.to_bytes());

        // For 5th PoKVC
        bytes.append(&mut self.pok_vc_4.to_bytes());

        bytes
    }


    /// Creates the final proof data after a Fiat-Shamir calculation
    pub fn gen_proof(
        self,
        challenge_hash: &TicketProofChallenge,
    ) -> Result<PoKOfTicketProof, BBSError> {
        let secrets_1: Vec<_> = self
        .pok_sig.secrets_1
        .iter()
        .map(|s| SignatureMessage(*s))
        .collect();
        let secrets_2: Vec<_> = self
        .pok_sig.secrets_2
        .iter()
        .map(|s| SignatureMessage(*s))
        .collect();
        let secrets_3: Vec<_> = self
            .secrets_3
            .iter()
            .map(|s| SignatureMessage(*s))
            .collect();
        let secrets_4: Vec<_> = self
            .secrets_4
            .iter()
            .map(|s| SignatureMessage(*s))
            .collect();
        let secrets_5: Vec<_> = self
            .secrets_5
            .iter()
            .map(|s| SignatureMessage(*s))
            .collect();
            
        let proof_vc_1 = self
            .pok_sig.pok_vc_1
            .gen_proof_ticket(challenge_hash, secrets_1.as_slice())?;

        let proof_vc_2 = self
            .pok_sig.pok_vc_2
            .gen_proof_ticket(challenge_hash, secrets_2.as_slice())?;

        let proof_vc_3 = self
            .pok_vc_3
            .gen_proof_ticket(challenge_hash, secrets_3.as_slice())?;

        let proof_vc_4 = self
            .pok_vc_4
            .gen_proof_ticket(challenge_hash, secrets_4.as_slice())?;
            
        let proof_vc_5 = self
            .pok_vc_5
            .gen_proof_ticket(challenge_hash, secrets_5.as_slice())?;

        Ok(PoKOfTicketProof {
            a_prime: self.pok_sig.a_prime,
            a_bar: self.pok_sig.a_bar,
            d: self.pok_sig.d,
            c: self.c,
            proof_vc_1,
            proof_vc_2,
            proof_vc_3,
            proof_vc_4,
            proof_vc_5,
        })
    }
}

impl PoKOfTicketProof{
        /// Convert the proof to raw bytes
        pub(crate) fn to_bytes(&self, compressed: bool) -> Vec<u8> {
            let mut output = Vec::new();
            self.a_prime.serialize(&mut output, compressed).unwrap();
            self.a_bar.serialize(&mut output, compressed).unwrap();
            self.d.serialize(&mut output, compressed).unwrap();
            self.c.serialize(&mut output, compressed).unwrap();

            let mut proof1_bytes = self.proof_vc_1.to_bytes(compressed);
            let proof1_len: u32 = proof1_bytes.len() as u32;
            output.extend_from_slice(&proof1_len.to_be_bytes()[..]);
            output.append(&mut proof1_bytes);

            let mut proof2_bytes = self.proof_vc_2.to_bytes(compressed);
            let proof2_len: u32 = proof2_bytes.len() as u32;
            output.extend_from_slice(&proof2_len.to_be_bytes()[..]);
            output.append(&mut proof2_bytes);

            let mut proof3_bytes = self.proof_vc_3.to_bytes(compressed);
            let proof3_len: u32 = proof3_bytes.len() as u32;
            output.extend_from_slice(&proof3_len.to_be_bytes()[..]);
            output.append(&mut proof3_bytes);

            let mut proof4_bytes = self.proof_vc_4.to_bytes(compressed);
            let proof4_len: u32 = proof4_bytes.len() as u32;
            output.extend_from_slice(&proof4_len.to_be_bytes()[..]);
            output.append(&mut proof4_bytes);


            let mut proof5_bytes = self.proof_vc_5.to_bytes(compressed);
            output.append(&mut proof5_bytes);
            output
        }

           /// Convert the byte slice into a proof
    pub(crate) fn from_bytes(
        data: &[u8],
        g1_size: usize,
        compressed: bool,
    ) -> Result<Self, BBSError> {
        if data.len() < g1_size * 3 {
            return Err(BBSError::from_kind(BBSErrorKind::PoKVCError {
                msg: format!("Invalid proof bytes. Expected {}", g1_size * 3),
            }));
        }
        let mut cursor = Cursor::new(data);

        let mut offset;
        let mut end = g1_size;
        let a_prime = slice_to_elem!(&mut cursor, G1, compressed)?;

        offset = end;
        end = offset + g1_size;
        let a_bar = slice_to_elem!(&mut cursor, G1, compressed)?;

        offset = end;
        end = offset + g1_size;
        let d = slice_to_elem!(&mut cursor, G1, compressed)?;

        offset = end;
        end = offset + g1_size;
        let c = slice_to_elem!(&mut cursor, G1, compressed)?;

        offset = end;
        end = offset + 4;
        let proof1_bytes = u32::from_be_bytes(*array_ref![data, offset, 4]) as usize;

        offset = end;
        end = offset + proof1_bytes;
        let proof_vc_1 = ProofG1::from_bytes(&data[offset..end], g1_size, compressed)?;


        offset = end;
        end = offset + 4;
        let proof2_bytes = u32::from_be_bytes(*array_ref![data, offset, 4]) as usize;

        offset = end;
        end = offset + proof2_bytes;
        let proof_vc_2 = ProofG1::from_bytes(&data[offset..end], g1_size, compressed)?;

        offset = end;
        end = offset + 4;
        let proof3_bytes = u32::from_be_bytes(*array_ref![data, offset, 4]) as usize;

        offset = end;
        end = offset + proof3_bytes;
        let proof_vc_3 = ProofG1::from_bytes(&data[offset..end], g1_size, compressed)?;

        offset = end;
        end = offset + 4;
        let proof4_bytes = u32::from_be_bytes(*array_ref![data, offset, 4]) as usize;

        offset = end;
        end = offset + proof4_bytes;
        let proof_vc_4 = ProofG1::from_bytes(&data[offset..end], g1_size, compressed)?;


        let proof_vc_5 = ProofG1::from_bytes(&data[end..], g1_size, compressed)?;
        Ok(Self {
            a_prime,
            a_bar,
            d,
            c,
            proof_vc_1,
            proof_vc_2,
            proof_vc_3,
            proof_vc_4,
            proof_vc_5,
        })
    }
}



impl ToVariableLengthBytes for PoKOfTicketProof {
    type Output = PoKOfTicketProof;
    type Error = BBSError;

    /// Convert the proof to a compressed raw bytes form.
    fn to_bytes_compressed_form(&self) -> Vec<u8> {
        self.to_bytes(true)
    }

    /// Convert compressed byte slice into a proof
    fn from_bytes_compressed_form<I: AsRef<[u8]>>(data: I) -> Result<Self, BBSError> {
        Self::from_bytes(data.as_ref(), G1_COMPRESSED_SIZE, true)
    }

    fn to_bytes_uncompressed_form(&self) -> Vec<u8> {
        self.to_bytes(false)
    }

    fn from_bytes_uncompressed_form<I: AsRef<[u8]>>(data: I) -> Result<Self::Output, Self::Error> {
        Self::from_bytes(data.as_ref(), G1_UNCOMPRESSED_SIZE, false)
    }
}



fn get_g2() -> G1{
    return G1::one();
}
