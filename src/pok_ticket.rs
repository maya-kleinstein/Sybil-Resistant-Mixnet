use std::{io::Cursor, collections::{BTreeMap, BTreeSet}, cmp::Ordering, fmt::{Display, Formatter, Result as FmtResult}};

use ff_zeroize::{Field, PrimeField};
use pairing_plus::{bls12_381::{G1, Fr, Bls12, G2, Fq12, FrRepr}, CurveProjective, serdes::SerDes, CurveAffine, Engine};

use crate::{prelude::*, rand_non_zero_fr, multi_scalar_mul_const_time_g1, hash_to_g1};

/// Indicates the status returned from `PoKOfSignatureProof`
#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PoKOfTicketProofStatus {
    /// The proof verified
    Success,
    /// The proof failed because the signature proof of knowledge failed
    BadSignature,
    /// The proof failed because a hidden message was invalid when the proof was created
    BadHiddenMessage,
    /// The proof failed because a revealed message was invalid
    BadRevealedMessage,
    /// The proof failed because the ticket proof of knowledge failed
    BadTicket
}

impl PoKOfTicketProofStatus {
    /// Return whether the proof succeeded or not
    pub fn is_valid(self) -> bool {
        matches!(self, PoKOfTicketProofStatus::Success)
    }
}

impl Display for PoKOfTicketProofStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match *self {
            PoKOfTicketProofStatus::Success => write!(f, "Success"),
            PoKOfTicketProofStatus::BadHiddenMessage => write!(
                f,
                "a message was supplied when the proof was created that was not signed or a message was revealed that was initially hidden"
            ),
            PoKOfTicketProofStatus::BadRevealedMessage => {
                write!(f, "a revealed message was supplied that was not signed or a message was revealed that was initially hidden")
            }
            PoKOfTicketProofStatus::BadSignature => {
                write!(f, "An invalid signature was supplied")
            }
            PoKOfTicketProofStatus::BadTicket =>{
                write!(f, "An invalid ticket was supplied")
            }
        }
    }
}

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
    /// A' in section 4.5
    a_prime: G1,
    /// \overline{A} in section 4.5
    a_bar: G1,
    /// d in section 4.5
    d: G1,
    /// C in my paper
    c : G1,
    /// For proving relation a_bar / d == a_prime^{-e} * h_0^r2
    pok_vc_1: ProverCommittedG1,
    /// The messages
    secrets_1: Vec<Fr>,
    /// For proving relation g1 * h1^m1 * h2^m2.... for all disclosed messages m_i == d^r3 * h_0^{-s_prime} * h1^-m1 * h2^-m2.... for all undisclosed messages m_i
    pok_vc_2: ProverCommittedG1,
    /// The blinding factors
    secrets_2: Vec<Fr>,
    /// revealed messages
    pub(crate) revealed_messages: BTreeMap<usize, SignatureMessage>,
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
        vk: &PublicKey,
        messages: &[ProofMessage],
        t: G1,
        b: G1,
    ) -> Result<Self, BBSError> {
        if messages.len() != vk.message_count() {
            return Err(BBSError::from_kind(
                BBSErrorKind::PublicKeyGeneratorMessageCountMismatch(
                    vk.message_count(),
                    messages.len(),
                ),
            ));
        }
        let sig_messages = messages
            .iter()
            .map(|m| m.get_message())
            .collect::<Vec<SignatureMessage>>();
        if !signature.verify(sig_messages.as_slice(), &vk)? {
            return Err(BBSErrorKind::PoKVCError {
                msg: "The messages and signature do not match.".to_string(),
            }
            .into());
        }

        let r1 = rand_non_zero_fr();
        let r2 = rand_non_zero_fr();

        let mut temp: Vec<SignatureMessage> = Vec::new();
        for i in 0..messages.len() {
            match &messages[i] {
                ProofMessage::Revealed(r) => temp.push(*r),
                ProofMessage::Hidden(HiddenMessage::ProofSpecificBlinding(m)) => temp.push(*m),
                ProofMessage::Hidden(HiddenMessage::ExternalBlinding(m, _)) => temp.push(*m),
            }
        }

        let b_sig = signature.get_b(temp.as_slice(), &vk);

        let mut a_prime = signature.a;
        a_prime.mul_assign(r1);

        let mut a_bar_denom = a_prime;
        a_bar_denom.mul_assign(signature.e);

        let mut a_bar = b_sig;
        a_bar.mul_assign(r1);
        a_bar.sub_assign(&a_bar_denom);

        let mut r2_d = r2;
        r2_d.negate();
        let mut builder = CommitmentBuilder::new();
        builder.add(&GeneratorG1(b_sig), &SignatureMessage(r1));
        builder.add(&vk.h0, &SignatureMessage(r2_d));

        // d = b^r1 h0^-r2
        let d = builder.finalize().0;

        let r3 = r1.inverse().unwrap();

        // s' = s - r2 r3
        let mut s_prime = r2;
        s_prime.mul_assign(&r3);
        s_prime.negate();
        s_prime.add_assign(&signature.s);

        // For proving relation a_bar / d == a_prime^{-e} * h_0^r2
        let mut committing_1 = ProverCommittingG1::new();
        let mut secrets_1 = Vec::with_capacity(2);
        // For a_prime^{-e}
        let blinding_e = rand_non_zero_fr();
        committing_1.commit_with(&GeneratorG1(a_prime), ProofNonce(blinding_e));
        let mut sig_e = signature.e;
        sig_e.negate();
        secrets_1.push(sig_e);
        // For h_0^r2
        committing_1.commit(&vk.h0);
        secrets_1.push(r2);
        let pok_vc_1 = committing_1.finish();

        // For proving relation g1 * h1^m1 * h2^m2.... for all disclosed messages m_i == d^r3 * h_0^{-s_prime} * h1^-m1 * h2^-m2.... for all undisclosed messages m_i
        // Usually the number of disclosed messages is much less than the number of hidden messages, its better to avoid negations in hidden messages and do
        // them in revealed messages. So transform the relation
        // g1 * h1^m1 * h2^m2.... * h_i^m_i for disclosed messages m_i = d^r3 * h_0^{-s_prime} * h1^-m1 * h2^-m2.... * h_j^-m_j for all undisclosed messages m_j
        // into
        // d^{-r3} * h_0^s_prime * h1^m1 * h2^m2.... * h_j^m_j = g1 * h1^-m1 * h2^-m2.... * h_i^-m_i. Moreover g1 * h1^-m1 * h2^-m2.... * h_i^-m_i is public
        // and can be efficiently computed as (g1 * h1^m1 * h2^m2.... * h_i^m_i)^-1 and inverse in elliptic group is a point negation which is very cheap
        let mut committing_2 = ProverCommittingG1::new();
        let mut secrets_2 = Vec::with_capacity(2 + messages.len());
        // For d^-r3
        committing_2.commit(&GeneratorG1(d));
        let mut r3_d = r3;
        r3_d.negate();
        secrets_2.push(r3_d);
        // h_0^s_prime
        committing_2.commit(&vk.h0);
        secrets_2.push(s_prime);

        let mut revealed_messages = BTreeMap::new();

        for i in 0..vk.message_count() {
            match &messages[i] {
                ProofMessage::Revealed(r) => {
                    revealed_messages.insert(i, *r);
                }
                ProofMessage::Hidden(HiddenMessage::ProofSpecificBlinding(m)) => {
                    committing_2.commit(&vk.h[i]);
                    secrets_2.push(m.0);
                }
                ProofMessage::Hidden(HiddenMessage::ExternalBlinding(e, b)) => {
                    committing_2.commit_with(&vk.h[i], b);
                    secrets_2.push(e.0);
                }
            }
        }
        let pok_vc_2 = committing_2.finish();


        let rho1 = rand_non_zero_fr();
        let rho2 = rand_non_zero_fr();

        let mut beta1 = rho1;
        beta1.mul_assign(&signature.e);

        let mut beta2 = rho2;
        beta2.mul_assign(&signature.e);

        let mut c = G1::one();
        c.mul_assign(rho1);
        let mut g2_rho2 = get_g2();
        g2_rho2.mul_assign(rho2);
        c.add_assign(&g2_rho2);

        
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
        committing_4.commit_with(&GeneratorG1(c), ProofNonce(blinding_e));
        let mut neg_sig_e = signature.e;
        neg_sig_e.negate();
        secrets_4.push(neg_sig_e);
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


        Ok(Self {
            a_prime,
            a_bar,
            d,
            c,
            pok_vc_1,
            secrets_1,
            pok_vc_2,
            secrets_2,
            revealed_messages,
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

        self.a_bar.serialize(&mut bytes, false).unwrap();

        // For 1st PoKVC
        // self.a_prime is included as part of self.pok_vc_1
        bytes.append(&mut self.pok_vc_1.to_bytes());

        // For 2nd PoKVC
        // self.d is included as part of self.pok_vc_2
        bytes.append(&mut self.pok_vc_2.to_bytes());

        // For 3rd PoKVC
        bytes.append(&mut self.pok_vc_3.to_bytes());

        // For 4th PoKVC
        // self.C is included as part of self.pok_vc_4
        bytes.append(&mut self.pok_vc_4.to_bytes());


        // For 5th PoKVC
        bytes.append(&mut self.pok_vc_5.to_bytes());

        bytes
    }


    /// Creates the final proof data after a Fiat-Shamir calculation
    pub fn gen_proof(
        self,
        challenge_hash: &ProofChallenge,
    ) -> Result<PoKOfTicketProof, BBSError> {
        let secrets_1: Vec<_> = self
            .secrets_1
            .iter()
            .map(|s| SignatureMessage(*s))
            .collect();
        let secrets_2: Vec<_> = self
            .secrets_2
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
            .pok_vc_1
            .gen_proof(challenge_hash, secrets_1.as_slice())?;

        let proof_vc_2 = self
            .pok_vc_2
            .gen_proof(challenge_hash, secrets_2.as_slice())?;

        let proof_vc_3 = self
            .pok_vc_3
            .gen_proof(challenge_hash, secrets_3.as_slice())?;

        let proof_vc_4 = self
            .pok_vc_4
            .gen_proof(challenge_hash, secrets_4.as_slice())?;
            
        let proof_vc_5 = self
            .pok_vc_5
            .gen_proof(challenge_hash, secrets_5.as_slice())?;

        Ok(PoKOfTicketProof {
            a_prime: self.a_prime,
            a_bar: self.a_bar,
            d: self.d,
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


    /// Return bytes that need to be hashed for generating challenge. Takes `self.a_bar`,
    /// `self.a_prime`, `self.d`, `self.c` and commitment and instance data of the five proof of knowledge protocols.
    pub fn get_bytes_for_challenge(
        &self,
        revealed_msg_indices: BTreeSet<usize>,
        vk: &PublicKey,
        b: G1,
        t: G1,
    ) -> Vec<u8> {

        let mut bytes = vec![];

        self.a_bar.serialize(&mut bytes, false).unwrap();
        self.a_prime.serialize(&mut bytes, false).unwrap();
        vk.h0.0.serialize(&mut bytes, false).unwrap();
        self.proof_vc_1
            .commitment
            .serialize(&mut bytes, false)
            .unwrap();
        self.d.serialize(&mut bytes, false).unwrap();
        vk.h0.0.serialize(&mut bytes, false).unwrap();
        for i in 0..vk.message_count() {
            if revealed_msg_indices.contains(&i) {
                continue;
            }
            vk.h[i].0.serialize(&mut bytes, false).unwrap();
        }
        self.proof_vc_2
            .commitment
            .serialize(&mut bytes, false)
            .unwrap();

        G1::one().serialize(&mut bytes, false).unwrap();
        get_g2().serialize(&mut bytes, false).unwrap();
        self.proof_vc_3
            .commitment
            .serialize(&mut bytes, false)
            .unwrap();
        
        self.c.serialize(&mut bytes, false).unwrap();
        G1::one().serialize(&mut bytes, false).unwrap();
        get_g2().serialize(&mut bytes, false).unwrap();
        self.proof_vc_4
            .commitment
            .serialize(&mut bytes, false)
            .unwrap();
        
        b.serialize(&mut bytes, false).unwrap();
        t.serialize(&mut bytes, false).unwrap();
        self.proof_vc_5
            .commitment
            .serialize(&mut bytes, false)
            .unwrap();
        
        bytes
    }

    /// Validate the proof
    pub fn verify(
        &self,
        vk: &PublicKey,
        revealed_msgs: &BTreeMap<usize, SignatureMessage>,
        challenge: &ProofChallenge,
        b: G1,
        t: G1,
    ) -> Result<PoKOfTicketProofStatus, BBSError> {
        vk.validate()?;
        for i in revealed_msgs.keys() {
            if *i >= vk.message_count() {
                return Err(BBSError::from_kind(BBSErrorKind::GeneralError {
                    msg: format!("Index {} should be less than {}", i, vk.message_count()),
                }));
            }
        }

        if self.a_prime.is_zero() {
            return Ok(PoKOfTicketProofStatus::BadSignature);
        }

        // Verifying the equation e(a_prime, w) = e(a_bar, g_2) 
        let mut a_bar = self.a_bar;
        a_bar.negate();
        match Bls12::final_exponentiation(&Bls12::miller_loop(&[
            (
                &self.a_prime.into_affine().prepare(),
                &vk.w.0.into_affine().prepare(),
            ),
            (
                &a_bar.into_affine().prepare(),
                &G2::one().into_affine().prepare(),
            ),
        ])) {
            None => return Ok(PoKOfTicketProofStatus::BadSignature),
            Some(product) => {
                if product != Fq12::one() {
                    return Ok(PoKOfTicketProofStatus::BadSignature);
                }
            }
        };


        // Verifying proof_vc_1
        let mut bases = vec![];
        bases.push(GeneratorG1(self.a_prime));
        bases.push(vk.h0);
        // a_bar / d
        let mut a_bar_d = self.a_bar;
        a_bar_d.sub_assign(&self.d);
        // let a_bar_d = &self.a_bar - &self.d;
        if !self
            .proof_vc_1
            .verify(&bases, &Commitment(a_bar_d), challenge)?
        {
            return Ok(PoKOfTicketProofStatus::BadHiddenMessage);
        }
        // TODO: future problem: maybe challenge should have borrow before it? for vc_2 as well?
        // Verifying proof_vc_2
        let mut bases_pok_vc_2 = Vec::with_capacity(2 + vk.message_count() - revealed_msgs.len());
        bases_pok_vc_2.push(GeneratorG1(self.d));
        bases_pok_vc_2.push(vk.h0);

        // `bases_disclosed` and `exponents` below are used to create g1 * h1^-m1 * h2^-m2.... for all disclosed messages m_i
        let mut bases_disclosed = Vec::with_capacity(1 + revealed_msgs.len());
        let mut exponents = Vec::with_capacity(1 + revealed_msgs.len());
        // XXX: g1 should come from a setup param and not generator
        bases_disclosed.push(G1::one());
        exponents.push(Fr::from_repr(FrRepr::from(1u64)).unwrap());
        for i in 0..vk.message_count() {
            if revealed_msgs.contains_key(&i) {
                let message = revealed_msgs.get(&i).unwrap();
                bases_disclosed.push(vk.h[i].0);
                exponents.push(message.0);
            } else {
                bases_pok_vc_2.push(vk.h[i]);
            }
        }
        // pr = g1 * h1^-m1 * h2^-m2.... = (g1 * h1^m1 * h2^m2....)^-1 for all disclosed messages m_i
        let mut pr = Commitment(multi_scalar_mul_const_time_g1(&bases_disclosed, &exponents));
        pr.0.negate();
        if !self
            .proof_vc_2
            .verify(bases_pok_vc_2.as_slice(), &pr, challenge)?
        {
            return Ok(PoKOfTicketProofStatus::BadHiddenMessage);
        }

        // Verifying proof_vc_3
        let bases = [GeneratorG1(G1::one()), GeneratorG1(get_g2())];
        if !self
            .proof_vc_3
            .verify(&bases, &Commitment(self.c), challenge)?
        {
            return Ok(PoKOfTicketProofStatus::BadTicket);
        }

        // Verifying proof_vc_4
        let bases = [GeneratorG1(self.c),GeneratorG1(G1::one()), GeneratorG1(get_g2())];
        if !self
            .proof_vc_4
            .verify(&bases, &Commitment(G1::zero()), challenge)?
        {
            return Ok(PoKOfTicketProofStatus::BadTicket);
        }

        // Verifying proof_vc_5
        let bases = [GeneratorG1(b), GeneratorG1(t)];
        if !self
            .proof_vc_5
            .verify(&bases, &Commitment(G1::zero()), challenge)?
        {
            return Ok(PoKOfTicketProofStatus::BadTicket);
        }

        // Testing if blindings for signature.e are equal
        assert_eq!(self.proof_vc_1.responses.first().cmp(&self.proof_vc_4.responses.first()), Ordering::Equal);
        
        // If everything worked!
        return Ok(PoKOfTicketProofStatus::Success);
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


/// Returns a generator of G1 that is different from g1
/// TODO: use lazy static to call once to generate the value (and not hash each time)
pub fn get_g2() -> G1{
    let bytes: [u8; 3] = [1, 2, 3];
    return hash_to_g1(&bytes);
}
