use ff_zeroize::Field;
use pairing_plus::{bls12_381::{G1, Fr}, CurveProjective};

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
        secrets_4.push(neg_rho1);
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
            .gen_proof_ticket(challenge_hash, secrets_3.as_slice())?;

        let proof_vc_2 = self
            .pok_sig.pok_vc_2
            .gen_proof_ticket(challenge_hash, secrets_3.as_slice())?;


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



fn get_g2() -> G1{
    return G1::one();
}
