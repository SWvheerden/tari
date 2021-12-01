# RFC-0241/XMR Atomic Swap

![status: draft](theme/images/status-draft.svg)

**Maintainer(s)**: [S W van Heerden](https://github.com/SWvheerden)

# Licence

[The 3-Clause BSD Licence](https://opensource.org/licenses/BSD-3-Clause).

Copyright 2021 The Tari Development Community

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
following conditions are met:

1. Redistributions of this document must retain the above copyright notice, this list of conditions and the following
   disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following
   disclaimer in the documentation and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products
   derived from this software without specific prior written permission.

THIS DOCUMENT IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS", AND ANY EXPRESS OR IMPLIED WARRANTIES,
INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
WHETHER IN CONTRACT, STRICT LIABILITY OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## Language

The keywords "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED",
"NOT RECOMMENDED", "MAY" and "OPTIONAL" in this document are to be interpreted as described in
[BCP 14](https://tools.ietf.org/html/bcp14) (covering RFC2119 and RFC8174) when, and only when, they appear in all capitals, as
shown here.

## Disclaimer

This document and its content are intended for information purposes only and may be subject to change or update
without notice.

This document may include preliminary concepts that may or may not be in the process of being developed by the Tari
community. The release of this document is intended solely for review and discussion by the community of the
technological merits of the potential system outlined herein.

## Goals

This Request for Comment (RFC) aims to describe how an Atomic swap between Tari and Monero will be created.

## Related Requests for Comment

* [RFC-0201: TariScript](RFC-0201_TariScript.md)
* [RFC-0202: TariScript Opcodes](RFC-0202_TariScriptOpcodes.md)

$$
\newcommand{\script}{\alpha} % utxo script
\newcommand{\input}{ \theta }
\newcommand{\cat}{\Vert}
\newcommand{\so}{\gamma} % script offset
\newcommand{\hash}[1]{\mathrm{H}\bigl({#1}\bigr)}
$$

## Description

Doing atomic swaps with Monero is more complicated and requires a crypto dance to complete as Monero does not
implement any form of HTLC's or the like. This means that when doing an atomic swap with Monero, most of the logic will
have to be implemented on the Tari side. Atomic swaps between Monero and bitcoin have been implemented by the [Farcaster  project](https://github.com/farcaster-project/RFCs)
and the [commit team](https://github.com/comit-network/xmr-btc-swap). Due to the way that TariScript works, we have a few advantages over the bitcoin script when it comes to [adaptor signatures](https://tlu.tarilabs.com/cryptography/introduction-to-scriptless-scripts#adaptor-signatures) as the [script key] was specifically designed with [scriptless scripts](https://tlu.tarilabs.com/cryptography/introduction-to-scriptless-scripts) in mind.

### Method

The primary, happy outline of a Tari - Monero atomic swap is described here, and more detail will follow. We will assume here that Alice wants to trade her XTR for Bob's XMR.

* Negotiation - Here, both parties negotiate about the values and how the Monero and Tari Utxo's will look
* Commitment - Here, both parties commit to their use of keys, and Bob commits to the refund transaction
* XTR payment - Here, the XTR payment is made to a multi-party UTXO containing a script
* XMR Payment - The Monero payment is made to a multiparty [scriptless script](https://tlu.tarilabs.com/cryptography/introduction-to-scriptless-scripts) UTXO.
* Claim XTR - Here, the XTR is claimed, and in claiming it, with the help of an adaptor signature, the XMR private key is revealed
* Claim XMR - Here, the XMR is claimed with the revealed key.

Please take note of the notation used in [TariScript] and specifically notation used on the signatures on the [transaction inputs](RFC-0201_TariScript.md#transaction-input-changes) and on the signatures on the [transaction outputs](RFC-0201_TariScript.md#transaction-output-changes), other notation will be noted in the [Notation](#notation) section.


### TL;DR

This scheme revolves around Alice, who wants to exchange some of her Tari for Bob's Monero. Because they don't 
trust each other, they have to commit some values to do the exchange. And if something goes wrong here, we want to ensure
that we can refund both parties either in Monero or Tari.

How this works is that Alice and Bob create a shared output on both chains. The Monero output is a simple aggregate key
to unlock the UTXO, while the Tari UTXO is unlocked by either one of three aggregate keys. The block height determines the unlock key on Tari. 

The Tari aggregate keys are constructed so that the swap transaction's signature will reveal Bob's Monero key
so that Alice has both keys, which allows her to claim the Monero. While the refund transaction's signature will reveal
Alice's Monero key so that Bob has both keys, and he claims the Monero.

To ensure that we can always claim the refund if Bob disappears after Alice posts the Tari UTXO, we need to
ensure that this refund transaction is completed and signed by both Alice and Bob before Alice publishes the Tari UTXO.
This ensures that in the case that Bob disappears, Alice can reclaim her Tari. And if Bob reappears, he can reclaim his Monero.

But in the case where Alice disappears after Bob posts the Monero transaction, we need to create a lapse transaction for
Bob to claim the Tari. This transaction is also completed and signed before the first Tari UTXO is published. This transaction
will reveal Bob's Monero key so that if Alice reappears, she can claim the Monero. 

![swap flow](assets/TXR_XMR_flow.png)

### TariScript

The Script used for the Tari UTXO is as follows:
``` TariScript,ignore
   CheckHeight(height_1)
   LtZero
   IFTHEN
      PushPubkey(K_{Ss})
   Else
      CheckHeight(height_2)
      LtZero
      IFTHEN
         PushPubkey(K_{Sr})
      Else
         PushPubkey(K_{Sl})
      ENDIF
   ENDIF
```

Here `height_1` is the lock height till Alice can claim the transaction. If Alice fails to publish the refund transaction
after `height_2,` Bob can claim the lapse transaction.

### Negotiation

Alice and Bob have to negotiate about the exchange rate and the amount to be exchanged in the atomic swap. 
They also need to decide how the two UTXO's will look on the blockchain. To accomplish this, the following needs to be finalized:

* Amount of Tari to swap for the amount of Monero
* Monero public key parts \\(X_a\\), \\(X_b\\) and its aggregate form \\(X\\)
* Tari [script key] parts \\(K_{Ssa}\\), \\(K_{Ssb}\\) and its aggregate form \\(K_{Ss}\\)
* Tari [script key] parts \\(K_{Sra}\\), \\(K_{Srb}\\) and its aggregate form \\(K_{Sr}\\)
* Tari [script key] parts \\(K_{Sla}\\), \\(K_{Slb}\\) and its aggregate form \\(K_{Sl}\\)
* Tari [sender offset key] parts  \\(K_{Osa}\\), \\(K_{Osb}\\) and its aggregate form \\(K_{Os}\\)
* Tari [sender offset key] parts  \\(K_{Ora}\\), \\(K_{Orb}\\) and its aggregate form \\(K_{Or}\\)
* Tari [sender offset key] parts  \\(K_{Ola}\\), \\(K_{Olb}\\) and its aggregate form \\(K_{Ol}\\)
* All of the nonces used in the script signature creation and Metadata signature for the swap, refund, and lapse transactions
* The [script offset] used in both the swap, refund, and lapse transactions
* The [TariScript] to be used in the Tari UTXO
* The blinding factor \\(k_i\\) for the Tari UTXO, this can be a Diffie-Hellman between their addresses.


### Key construction

Using multi-signatures with Schnorr signatures, we need to ensure that the keys are constructed so that key
cancellation attacks are not possible. To do this, we follow the Musig way of creating keys. 
Musig keys are constructed in the following way if there are two parties.

$$
\begin{aligned}
K_a &=  \hash{\hash{K_a' \cat K_b'} \cat K_a' } * K_a' \\\\
k_a &=  \hash{\hash{K_a' \cat K_b'} \cat K_a' } * k_a' \\\\
K_b &=  \hash{\hash{K_a' \cat K_b'} \cat K_b' } * K_b' \\\\
k_b &=  \hash{\hash{K_a' \cat K_b'} \cat K_b' } * k_b' \\\\
\end{aligned}
\tag{1}
$$

The [script key] parts for Alice and Bob is constructed as follows:

$$
\begin{aligned}
k_{Ssa} &=  \hash{\hash{K_{Ssa}' \cat K_{Ssb}'} \cat K_{Ssa}' } * k_{Ssa}' \\\\
k_{Ssb} &=  \hash{\hash{K_{Ssa}' \cat K_{Ssb}'} \cat K_{Ssb}' } * k_{Ssb}' \\\\
k_{Ss} &= k_{Ssa} + k_{sb} \\\\
k_{Sra} &=  \hash{\hash{K_{Sra}' \cat K_{Srb}'} \cat K_{Sra}' } * k_{Sra}' \\\\
k_{Srb} &=  \hash{\hash{K_{Sra}' \cat K_{Srb}'} \cat K_{Srb}' } * k_{Srb}' \\\\
k_{Sr} &= k_{Sra} + k_{Srb} \\\\
k_{Sla} &=  \hash{\hash{K_{Sla}' \cat K_{Slb}'} \cat K_{Sla}' } * k_{Sla}' \\\\
k_{Slb} &=  \hash{\hash{K_{Sla}' \cat K_{Slb}'} \cat K_{Slb}' } * k_{Slb}' \\\\
k_{Sl} &= k_{Sla} + k_{Slb} \\\\
\end{aligned}
\tag{2}
$$

The [sender offset key] parts for Alice and Bob is constructed as follows:

$$
\begin{aligned}
k_{Osa} &=  \hash{\hash{K_{Osa}' \cat K_{Osb}'} \cat K_{Osa}' } * k_{Osa}' \\\\
k_{Osb} &=  \hash{\hash{K_{Osa}' \cat K_{Osb}'} \cat K_{Osb}' } * k_{Osb}' \\\\
k_{Os} &= k_{Osa} + k_{Ssb} \\\\
k_{Ora} &=  \hash{\hash{K_{Ora}' \cat K_{Orb}'} \cat K_{Ora}' } * k_{Ora}' \\\\
k_{Orb} &=  \hash{\hash{K_{Ora}' \cat K_{Orb}'} \cat K_{Orb}' } * k_{Orb}' \\\\
k_{Or} &= k_{Ora} + k_{Srb} \\\\
k_{Ola} &=  \hash{\hash{K_{Ola}' \cat K_{Olb}'} \cat K_{Ola}' } * k_{Ola}' \\\\
k_{Olb} &=  \hash{\hash{K_{Ola}' \cat K_{Olb}'} \cat K_{Olb}' } * k_{Olb}' \\\\
k_{Ol} &= k_{Ola} + k_{Slb} \\\\
\end{aligned}
\tag{3}
$$

The Monero key parts for Alice and Bob is constructed as follows:

$$
\begin{aligned}
x_a &=  \hash{\hash{X_a' \cat X_b'} \cat X_a' } * x_a' \\\\
x_b &=  \hash{\hash{X_a' \cat X_b'} \cat X_b' } * x_b' \\\\
x &= x_a + x_b \\\\
\end{aligned}
\tag{4}
$$


### Commitment phase

This phase allows Alice and Bob to commit to the use of their keys. This phase requires more than one round to complete
as some of the information that needs to be committed to is dependent on previous knowledge. 

Alice needs to provide Bob the following:

* Output commitment \\(C_r\\) of the refund transaction's output
* Output features  \\( F_r\\) of the refund transaction's output
* Output script  \\( \\script_r\\) of the refund transaction's output
* Output commitment \\(C_l\\) of the lapse transaction's output
* Output features  \\( F_l\\) of the lapse transaction's output
* Output script  \\( \\script_l\\) of the lapse transaction's output
* Public keys: \\( K_{Ssa}'\\), \\( K_{Sra}'\\), \\( K_{Sla}'\\), \\( K_{Osa}'\\), \\( K_{Ora}'\\), \\( K_{Ola}'\\), \\( X_a'\\)
* Nonces: \\( R_{Ssa}\\), \\( R_{Sra}\\), \\( R_{Sla}\\), \\( R_{Msa}\\), \\( R_{Mra}\\), \\( R_{Mla}\\)

Bob needs to provide Alice the following:

* Output commitment \\(C_s\\) of the swap transaction's output
* Output features  \\( F_s\\) of the swap transaction's output
* Output script  \\( \\script_s\\) of the swap transaction's output
* Public keys: \\( K_{Ssb}'\\), \\( K_{Srb}'\\), \\( K_{Slb}'\\), \\( K_{Osb}'\\), \\( K_{Orb}'\\), \\( K_{Olb}'\\), \\( X_b'\\)
* Nonces: \\( R_{Ssb}\\), \\( R_{Srb}\\), \\( R_{Slb}\\), \\( R_{Msb}\\), \\( R_{Mrb}\\), \\( R_{Mlb}\\)

After both Alice and Bob have exchanged the variables, they start trading calculated values.

Alice needs to provide Bob with the following values:

* Adaptor signature part \\(b_{Sra}'\\) for \\(b_{Sra}\\)
* Signature part \\(a_{Sra}\\)
* Monero public key \\(X_a\\) on Ristretto 
* Monero public key \\(Xm_a\\) on ed25519
* Zero Knowledge proof for \\(x_a == xm_a\\): \\((R_{ZTa}, s_{ZTa})\\) and \\((R_{ZMa}, s_{ZMa})\\) 

Alice constructs  \\(a_{Sra}\\) and \\(b_{Sra}\\)' with
$$
\begin{aligned}
a_{Sra} &= r_{Sra_a} +  e_r(v_{i}) \\\\
b_{Sra}' &= r_{Sra_b} +  e_r(k_{Sra}+k_i) \\\\
e_r &= \hash{ (R_{Sr} + (X_a)) \cat \alpha_r \cat \input_r \cat (K_{Sra} + K_{Srb}) \cat C_i} \\\\
R_{Sr} &= r_{Sra_a} \cdot H + r_{Sra_b} \cdot G + R_{Srb} \\\\
X_a &= x_a \cdot G \\\\
\end{aligned}
\tag{5}
$$

Alice constructs the Zero Knowledge proof for \\(x_a == xm_a\\) with:

$$
\begin{aligned}
e = \hash{X_a \cat XM_a \cat R_{ZTa} \cat R_{ZMa}}
s_{ZTa} = r_{ZTa} + e(x_a)
s_{ZMa} = r_{ZMa} + e(xm_a)
\end{aligned}
\tag{6}
$$

Bob needs to provide Alice with the following values:

* Adaptor signature part \\(b_{Ssb}'\\) for \\(b_{Ssb}\\)
* Signature part \\(a_{Ssb}\\)
* Adaptor signature part \\(b_{Slb}'\\) for \\(b_{Slb}\\)
* Signature part \\(a_{Slb}\\)
* Monero public key \\(X_b\\) on Ristretto 
* Monero public key \\(Xm_b\\) on ed25519
* Zero Knowledge proof for \\(x_b == xm_b\\): \\((R_{ZTb}, s_{ZTb})\\) and \\((R_{ZMb}, s_{ZMb})\\) 

Bob constructs \\(a_{Ssb}\\), \\(b_{Ssb}'\\), \\(a_{Slb}\\) and \\(b_{Slb}'\\) with
$$
\begin{aligned}
a_{Ssb} &= r_{Ssb_a} +  e_s(v_{i}) \\\\
b_{Ssb}' &= r_{Ssb_b} +  e_s(k_{Ssb}+k_i) \\\\
e_s &= \hash{ (R_{Sr} + (X_b)) \cat \alpha_i \cat \input_i \cat (K_{Ssa} + K_{Ssb}) \cat C_i} \\\\
R_{Ss} &= r_{Ssb_a} \cdot H + r_{Ssb_b} \cdot G + R_{Ssa} \\\\
a_{Slb} &= r_{Slb_a} +  e_l(v_{i}) \\\\
b_{Slb}' &= r_{Slb_b} +  e_l(k_{Slb}+k_i) \\\\
e_l &= \hash{ (R_{Sl} + (X_b)) \cat \alpha_i \cat \input_i \cat (K_{Sla} + K_{Slb}) \cat C_i} \\\\
R_{Sl} &= r_{Slb_a} \cdot H + r_{Slb_b} \cdot G + R_{Sla} \\\\
X_b &= x_b \cdot G \\\\
\end{aligned}
\tag{7}
$$

Bob constructs the Zero Knowledge proof for \\(x_b == xm_b\\) with:

$$
\begin{aligned}
e = \hash{X_b \cat XM_b \cat R_{ZTb} \cat R_{ZMb}}
s_{ZTb} = r_{ZTb} + e(x_b)
s_{ZMb} = r_{ZMb} + e(xm_b)
\end{aligned}
\tag{8}
$$

Alice needs to verify Bob's adaptor signatures with:

$$
\begin{aligned}
a_{Ssb} \cdot H + b_{Ssb}' \cdot G &= R_{Ssb} + (C_i+K_{Ssb})*e_s \\\\
a_{Slb} \cdot H + b_{Slb}' \cdot G &= R_{Slb} + (C_i+K_{Slb})*e_l \\\\
\end{aligned}
\tag{9}
$$

Alice needs to verify Bob's Monero public keys with:

$$
\begin{aligned}
e = \hash{X_b \cat XM_b \cat R_{ZTb} \cat R_{ZMb}}
s_{ZTb} \cdot G &= R_{ZTb} + e(X_b) \\\\
s_{ZMb} \cdot M &= R_{ZMb} + e(Xm_b) \\\\
R_{ZTb} - R_{ZMb} &= s_{ZTb} - s_{ZMb}
\end{aligned}
\tag{10}
$$

Bob needs to verify Alice's adaptor signature with:

$$
\begin{aligned}
a_{Sra} \cdot H + b_{Sra}' \cdot G &= R_{Sra} + (C_i+K_{Sra})*e_r \\\\
\end{aligned}
\tag{11}
$$

Bob needs to verify Alice's Monero public keys with:

$$
\begin{aligned}
e = \hash{X_a \cat XM_a \cat R_{ZTa} \cat R_{ZMa}}
s_{ZTa} \cdot G &= R_{ZTa} + e(X_a) \\\\
s_{ZMa} \cdot M &= R_{ZMa} + e(Xm_a) \\\\
R_{ZTa} - R_{ZMa} &= s_{ZTa} - s_{ZMa}
\end{aligned}
\tag{12}
$$

If Alice and Bob are happy with the verification, they need to swap out refund and lapse transactions.

Alice needs to provide Bob with the following:

* Script Signature for lapse transaction (\\( (a_{Sla}, b_{Sla}), R_{Sla}\\) )
* Metadata signature for lapse transaction (\\( b_{Mla}, R_{Mla}\\) )
* Script offset for lapse transaction \\( \so_{la} \\)

Alice constructs for the lapse transaction signatures.
$$
\begin{aligned}
a_{Sla} &= r_{Sla_a} +  e_l(v_{i}) \\\\
b_{Sla} &= r_{Sla_b} +  e_l(k_{Sla}) \\\\
e_l &= \hash{ (R_{Sl} + (X_b)) \cat \alpha_i \cat \input_i \cat (K_{Sla} + K_{Slb}) \cat C_i} \\\\
R_{Sl} &= r_{Sla_a} \cdot H + r_{Sla_b} \cdot G + R_{Slb}\\\\
b_{Mla} &= r_{Mla_b} + e(k\_{Ola}) \\\\
R_{Mla} &= b_{Mla} \cdot G \\\\
e &= \hash{ (R_{Mla} + R_{Mlb}) \cat \script_l \cat F_l \cat (K_{Ola} + K_{Olb}) \cat C_l} \\\\
\so_{la} &= k_{Sla} - k_{Ola}
\end{aligned}
\tag{13}
$$

Bob needs to provide Alice with the following:

* Script Signature for refund transaction (\\( (a_{Srb}, b_{Srb}), R_{Srb}\\) )
* Metadata signature for refund transaction (\\( b_{Mrb}, R_{Mrb}\\) )
* Script offset for refund transaction \\( \so_{rb} \\)

Bob constructs for the refund transaction signatures.
$$
\begin{aligned}
a_{Srb} &= r_{Srb_a} +  e_r(v_{i}) \\\\
b_{Srb} &= r_{Srb_b} +  e_r(k_{Srb}) \\\\
e_r &= \hash{ (R_{Sr} + (X_a)) \cat \alpha_i \cat \input_i \cat (K_{Sra} + K_{Srb}) \cat C_i} \\\\
R_{Sl} &= r_{Srb_a} \cdot H + r_{Srb_b} \cdot G + R_{Sra}\\\\
b_{Mrb} &= r_{Mrb_b} + e(k\_{Orb}) \\\\
R_{Mrb} &= b_{Mrb} \cdot G \\\\
e &= \hash{ (R_{Mra} + R_{Mrb}) \cat \script_r \cat F_r \cat (K_{Ora} + K_{Orb}) \cat C_r} \\\\
\so_{rb} &= k_{Srb} - k_{Orb}
\end{aligned}
\tag{14}
$$

Although the script validation on output \\(C_i\\)  will not pass due to the lock height, both Alice and Bob need to
verify that the total aggregated signatures and script offset for the refund and lapse transaction are valid should they
need to publish them at a future date without the presence of the other party.

### XTR payment

If Alice and Bob are happy with all the committed values up to now. Alice will create a Tari UTXO with the script 
mentioned above. And because Bob already gave her the required signatures for his part of the refund transaction, Alice 
can easily compute the required aggregated signatures by adding the parts together so that she has all the 
knowledge to spend this after the lock expires. 

### XMR Payment

If Bob cab see that Alice has published the Tari UTXO with the correct script, Bob can go ahead and publish the Monero UTXO
with the aggregate key \\(X = X_a + X_b \\).

### Claim XTR 

If Alice can see that Bob published the Monero UTXO to the correct aggregate key \\(X\\). She does not yet have the required
key \\(x_b \\) to claim the Monero. 
But she can now provide Bob with the following allowing him to spend the Tari UTXO:

* Script signature for the swap transaction \\((a_{Ssa}\\, b_{Ssa}), R_{Ssa}\\)
* Metadata signature for swap transaction (\\ b_{Msa}, R_{Msa}\\ )
* Script offset for swap transaction \\( \so_{sa} \\)

Alice constructs for the swap transaction.
$$
\begin{aligned}
a_{Ssa} &= r_{Ssa_a} +  e_s(v_{i}) \\\\
b_{Ssa} &= r_{Ssa_b} +  e_s(k_{Ssa}) \\\\
e_s &= \hash{ (R_{Ss} + (X_b)) \cat \alpha_i \cat \input_i \cat (K_{Ssa} + K_{Ssb}) \cat C_i} \\\\
R_{Ss} &= r_{Ssa_a} \cdot H + r_{Ssa_b} \cdot G + R_{Ssb}\\\\
b_{Msa} &= r_{Msa_b} + e(k\_{Osa}) \\\\
R_{Msa} &= b_{Msa} \cdot G \\\\
e &= \hash{ (R_{Msa} + R_{Msb}) \cat \script_s \cat F_s \cat (K_{Osa} + K_{Osb}) \cat C_s} \\\\
\so_{sa} &= k_{Ssa} - k_{Osa} \\\\
\end{aligned}
\tag{15}
$$

Bob constructs the swap transaction.
$$
\begin{aligned}
a_{Ssb} &= r_{Ssb_a} +  e_s(v_{i}) \\\\
b_{Ssb} &= r_{Ssb_b} + x_b + e_s(k_{Ssb} + k_i) \\\\
e_s &= \hash{ (R_{Ss} + (X_b)) \cat \alpha_i \cat \input_i \cat (K_{Ssa} + K_{Ssb}) \cat C_i} \\\\
a_{Ss} &= a_{Ssa} + a_{Ssb} \\\\
b_{Ss} &= b_{Ssa} + b_{Ssb} \\\\
R_{Ss} &= r_{Ssa_b} \cdot H + r_{Ssb_b} \cdot G + R_{Ssa}\\\\
a_{Msb} &= r_{Msb_a} + e(v_{s}) \\\\
b_{Msb} &= r_{Msb_b} + e(k\_{Osb}+k_s) \\\\
R_{Msb} &= a_{Msb} \cdot H + b_{Msb} \cdot G \\\\
e &= \hash{ (R_{Msa} + R_{Msb}) \cat \script_s \cat F_s \cat (K_{Osa} + K_{Osb}) \cat C_s} \\\\
R_{Ms} &= R_{Msa} + R_{Msb} \\\\
\so_{sb} &= k_{Ssb} - k_{Osb} \\\\
\so_{s} &= \so_{sa} +\so_{sb} \\\\
\end{aligned}
\tag{16}
$$

Bob's transaction now has all the required signatures to complete the transaction. He will then publish the transaction.

### Claim XMR

Because Bob has now published the transaction on the Tari blockchain, Alice can calculate the missing Monero key \\(x_b\\)
this she does with:
$$
\begin{aligned}
b_{Ss} &= b_{Ssa} + b_{Ssb} \\\\
b_{Ss} - b_{Ssa} &= b_{Ssb} \\\\
b_{Ssb} &= r_{Ssb_b} + x_b + e_s(k_{Ssb} + k_i) \\\\
b_{Ssb} - b_{Ssb}' &= r_{Ssb_b} + x_b + e_s(k_{Ssb} + k_i) -(r_{Ssb_b} +  e_s(k_{Ssb}+k_i))\\\\
b_{Ssb} - b_{Ssb}' &= x_b \\\\
\end{aligned}
\tag{17}
$$

With \\(x_b\\) in hand she can calculate \\(X = x_a + x_b\\) and with this she claim the Monero.

### The refund

If something goes wrong and Bob never publishes the Monero, or he disappears. Alice needs to wait for the lock height
`height_1` to pass. This will allow her to create the refund transaction to reclaim her Tari.  

Alice constructs the refund transaction with
$$
\begin{aligned}
a_{Sra} &= r_{Sra_a} +  e_s(v_{i}) \\\\
b_{Sra} &= r_{Sra_b} + x_a + e_s(k_{Sra} + k_i) \\\\
e_r &= \hash{ (R_{Sr} + (X_a)) \cat \alpha_i \cat \input_i \cat (K_{Sra} + K_{Srb}) \cat C_i} \\\\
a_{Sr} &= a_{Sra} + a_{Srb} \\\\
b_{Sr} &= b_{Sra} + b_{Srb} \\\\
R_{Sr} &= r_{Sra_a} \cdot H + r_{Sra_b} \cdot G + R_{Srb}\\\\
a_{Mra} &= r_{Mra_a} + e(v_{r}) \\\\
b_{Mra} &= r_{Mra_b} + e(k\_{Ora}+k_r) \\\\
R_{Mra} &= a_{Mra} \cdot H + b_{Mra} \cdot G \\\\
e &= \hash{ (R_{Mra} + R_{Mrb}) \cat \script_s \cat F_s \cat (K_{Ora} + K_{Orb}) \cat C_r} \\\\
R_{Mr} &= R_{Mra} + R_{Mrb} \\\\
\so_{ra} &= k_{Sra} - k_{Ora} \\\\
\so_{r} &= \so_{ra} +\so_{rb} \\\\
\end{aligned}
\tag{18}
$$

This allows Alice to claim back her Tari, but it also exposes her Monero key \\(x_a\\)
This means if Bob did publish the Monero UTXO, he could calculate \\(X\\) using:
$$
\begin{aligned}
b_{Sr} &= b_{Sra} + b_{Srb} \\\\
b_{Sr} - b_{Sra} &= b_{Sra} \\\\
b_{Sra} &= r_{Sra_b} + x_a + e_r(k_{Sra} + k_i) \\\\
b_{Sra} - b_{Sra}' &= r_{Sra_b} + x_a + e_r(k_{Sra} + k_i) -(r_{Sra_b} +  e_r(k_{Sra}+k_i))\\\\
b_{Sra} - b_{Sra}' &= x_a \\\\
\end{aligned}
\tag{19}
$$


### The lapse transaction

If something goes wrong and Alice never publishes her refund transition and or she disappears. Bob needs to wait for the lock height
`height_2` to pass. This will allow him to create the lapse transaction to claim the Tari. 

Bob constructs the lapse transaction with
$$
\begin{aligned}
a_{Slb} &= r_{Slb_a} +  e_l(v_{i}) \\\\
b_{Slb} &= r_{Slb_b} + x_b + e_l(k_{Slb} + k_i) \\\\
e_l &= \hash{ (R_{Sl} + (X_b)) \cat \alpha_i \cat \input_i \cat (K_{Sla} + K_{Slb}) \cat C_i} \\\\
a_{Sl} &= a_{Sla} + a_{Slb} \\\\
b_{Sl} &= b_{Sla} + b_{Slb} \\\\
R_{Sl} &= r_{Slb_a} \cdot H + r_{Slb_b} \cdot G + R_{Sla}\\\\
a_{Mlb} &= r_{Mlb_a} + e(v_{l}) \\\\
b_{Mlb} &= r_{Mlb_b} + e(k\_{Olb}+k_l) \\\\
R_{Mlb} &= a_{Mlb} \cdot H + b_{Mlb} \cdot G \\\\
e &= \hash{ (R_{Mla} + R_{Mlb}) \cat \script_l \cat F_l \cat (K_{Ola} + K_{Olb}) \cat C_l} \\\\
R_{Ml} &= R_{Mla} + R_{Mlb} \\\\
\so_{lb} &= k_{Slb} - k_{Olb} \\\\
\so_{r} &= \so_{la} +\so_{lb} \\\\
\end{aligned}
\tag{20}
$$

This allows Bob to claim the Tari he originally wanted, but it also exposes his Monero key \\(x_b\\)
This means if Alice ever comes back online, she can calculate \\(X\\) and claim the Monero she wanted all along using:
$$
\begin{aligned}
b_{Sl} &= b_{Slb} + b_{Slb} \\\\
b_{Sl} - b_{Slb} &= b_{Slb} \\\\
b_{Slb} &= r_{Slb_b} + x_a + e_r(k_{Slb} + k_i) \\\\
b_{Slb} - b_{Slb}' &= r_{Slb_b} + x_b + e_r(k_{Slb} + k_i) -(r_{Slb_b} +  e_r(k_{Slb}+k_i))\\\\
b_{Slb} - b_{Slb}' &= x_b \\\\
\end{aligned}
\tag{21}
$$

## Alternative approach

### Alternative approach Description

Doing atomic swaps with Monero is more complicated and requires a crypto dance to complete as Monero does not
implement any form of HTLC's or the like. This means that when doing an atomic swap with Monero, most of the logic will
have to be implemented on the Tari side. Atomic swaps between Monero and bitcoin have been implemented by the [Farcaster  project](https://github.com/farcaster-project/RFCs)
and the [commit team](https://github.com/comit-network/xmr-btc-swap).

### Alternative approach Method

The primary, happy outline of a Tari - Monero atomic swap is described here, and more detail will follow. We will assume here that Alice wants to trade her XTR for Bob's XMR.

* Negotiation - Here, both parties negotiate about the values and how the Monero and Tari Utxo's will look
* Commitment - Here, both parties commit to their use of keys, and Bob commits to the refund transaction
* XTR payment - Here, the XTR payment is made to a multi-party UTXO containing a script
* XMR Payment - The Monero payment is made to a multiparty [scriptless script](https://tlu.tarilabs.com/cryptography/introduction-to-scriptless-scripts) UTXO.
* Claim XTR - Here, the XTR is claimed, and in claiming it, the XMR private key is revealed
* Claim XMR - Here, the XMR is claimed with the revealed key.

Please take note of the notation used in [TariScript] and specifically notation used on the signatures on the [transaction inputs](RFC-0201_TariScript.md#transaction-input-changes) and on the signatures on the [transaction outputs](RFC-0201_TariScript.md#transaction-output-changes), other notation will be noted in the [Notation](#notation) section.


### Alternative approach TL;DR

This scheme revolves around Alice, who wants to exchange some of her Tari for Bob's Monero. Because they don't 
trust each other, they have to commit some values to do the exchange. And if something goes wrong here, we want to ensure
that we can refund both parties either in Monero or Tari.

How this works is that Alice and Bob create a shared output on both chains. The Monero output is a simple aggregate key
to unlock the UTXO, while the Tari UTXO is unlocked by either one of two keys. The block height determines the unlock key on Tari. 

The TariScript will decide the unlock key to claim the Tari amount. The script will require the user to input their part
of the Monero aggregate key used to lock the Monero UTXO. The script can end one of three ways, one for the happy path if
Alice gives Bob the pre_image after she checked and verified the Monero UTXO, one for her to reclaim her Tari amount if Bob
disappears or tries to break the contract. And lastly, one for Bob to claim the Tari if Alice disappears after he publishes the
Monero UTXO.

### Alternative approach TariScript

The Script used for the Tari UTXO is as follows:
``` TariScript,ignore
   CheckHeight(height_1)
   LtZero
   IFTHEN
      HashSha256 
      PushHash(HASH256{pre_image})
      EqualVerify
      Ristretto
      PushPubkey(X_b)
      EqualVerify
      PushPubkey(K_{Sb})
   Else
      CheckHeight(height_2)
      LtZero
      IFTHEN
         Ristretto
         PushPubkey(X_a)
         EqualVerify
         PushPubkey(K_{Sa})
      Else
         Ristretto
         PushPubkey(X_b)
         EqualVerify
         PushPubkey(K_{Sb})
      ENDIF
   ENDIF
```

Here `height_1` is the lock height till Alice can claim the transaction. If Alice fails to publish the refund transaction
after `height_2,` Bob can claim the lapse transaction.

### Alternative approach Negotiation

Alice and Bob have to negotiate the exchange rate and the amount to be exchanged in the atomic swap. 
They also need to decide how the two UTXO's will look on the blockchain. To accomplish this, the following needs to be finalized:

* Amount of Tari to swap for the amount of Monero
* Monero public key parts \\(X_a\\), \\(X_b\\) and its aggregate form \\(X\\)
* Tari [script key] parts \\(K_{Sa}\\), \\(K_{Sb}\\) 
* The [TariScript] to be used in the Tari UTXO
* The blinding factor \\(k_i\\) for the Tari UTXO, this can be a Diffie-Hellman between their addresses.


### Alternative approach Key construction

We need to use multi-signatures with Schnorr signatures to ensure that the keys are constructed so that key
cancellation attacks are not possible. To do this, we follow the Musig way of creating keys. 
Musig keys are constructed in the following way if there are two parties.

$$
\begin{aligned}
K_a &=  \hash{\hash{K_a' \cat K_b'} \cat K_a' } * K_a' \\\\
k_a &=  \hash{\hash{K_a' \cat K_b'} \cat K_a' } * k_a' \\\\
K_b &=  \hash{\hash{K_a' \cat K_b'} \cat K_b' } * K_b' \\\\
k_b &=  \hash{\hash{K_a' \cat K_b'} \cat K_b' } * k_b' \\\\
\end{aligned}
\tag{22}
$$

The Monero key parts for Alice and Bob is constructed as follows:


$$
\begin{aligned}
x_a &=  \hash{\hash{X_a' \cat X_b'} \cat X_a' } * x_a' \\\\
x_b &=  \hash{\hash{X_a' \cat X_b'} \cat X_b' } * x_b' \\\\
x &= x_a + x_b \\\\
\end{aligned}
\tag{23}
$$


### Alternative approach Commitment phase

This phase allows Alice and Bob to commit to the use of their keys. This phase requires more than one round to complete
as some of the information that needs to be committed to is dependent on previous knowledge. 

Alice needs to provide Bob the following:

* Script key  \\( \\k_{Sa}\\)
* Monero public key:  \\( X_a'\\)

Bob needs to provide Alice the following:

* Script key  \\( \\k_{Sb}\\)
* Monero public key:  \\( X_b'\\)

After this exchange of values, Alice needs to provide Bob with the following:

* Monero public key \\(X_a\\) on Ristretto 
* Monero public key \\(Xm_a\\) on ed25519
* Zero Knowledge proof for \\(x_a == xm_a\\): \\((R_{ZTa}, s_{ZTa})\\) and \\((R_{ZMa}, s_{ZMa})\\) 

Bob needs to provide Alice with the following:

* Monero public key \\(X_b\\) on Ristretto 
* Monero public key \\(Xm_b\\) on ed25519
* Zero Knowledge proof for \\(x_b == xm_b\\): \\((R_{ZTb}, s_{ZTb})\\) and \\((R_{ZMb}, s_{ZMb})\\) 

The construction and verification of the Zero knowledge proofs are shown in (6), (8), (10) and (12)

### Alternative approach XTR payment

Alice will construct the Tari UTXO and publish this to the blockchain, knowing that she can reclaim her Tari if Bob vanishes
or tried to break the agreement.


### Alternative approach XMR Payment

If Bob cab see that Alice has published the Tari UTXO with the correct script, Bob can go ahead and publish the Monero UTXO
with the aggregate key \\(X = X_a + X_b \\).

### Alternative approach Claim XTR 

If Alice can see that Bob published the Monero UTXO to the correct aggregate key \\(X\\). She does not yet have the required
key \\(x_b \\) to claim the Monero. 
But she can now provide Bob with the correct pre_image to spend the Tari UTXO.

Bob can now supply the pre_image, and he has to give his Monero private key to the transaction to unlock the script.

### Alternative approach Claim XMR

Alice can now see that Bob spent the Tari UTXO, and by looking at the input_data required to spend the script, she can learn
Bob's secret Monero key. Although this key is public, her part of the Monero spend key is still private, and thus only she
knows the complete Monero spend key. She can use this knowledge to claim the Monero UTXO.

### Alternative approach The refund

If something goes wrong and Bob never publishes the Monero, or he disappears. Alice needs to wait for the lock height
`height_1` to pass. This will allow her to reclaim her Tari. But in doing so, she needs to publish her Monero
secret key as input to the TariScript to unlock the Tari. In doing so, when Bob comes back online, he can use
this knowledge to reclaim his Monero as only he now knows both parts of the Monero UTXO spend key.


### Alternative approach The lapse transaction

If something goes wrong and Alice never gives Bob the preimage, or she disappears. Bob needs to wait for the lock height
`height_2` to pass. This will allow him to create claim the Tari he wanted all along. But in doing so, he needs to publish
his Monero secret key as input to the TariScript to unlock the Tari. In doing so, when Alice comes back online,
he can use this knowledge to claim the Monero she wanted all along as only she now knows both parts of the Monero UTXO spend key.

## Notation

Where possible, the "usual" notation is used to denote terms commonly found in cryptocurrency literature. Lower case 
characters are used as private keys, while uppercase characters are used as public keys. New terms introduced here are 
assigned greek lowercase letters in most cases. Some terms used here are noted down in [TariScript]. 

| Name                        | Symbol                | Definition |
|:----------------------------|-----------------------| -----------|
| Monero key                  | \\( X \\)             | Alice's partial  Monero public key  on Ristretto |
| Alice's Monero key          | \\( X_a \\)           | Alice's partial  Monero public key on Ristretto |
| Bob's Monero key            | \\( X_b \\)           | Bob's partial  Monero public key on Ristretto   |
| Monero key                  | \\( Xm \\)            | Alice's partial  Monero public key  on ed25519 |
| Alice's Monero key          | \\( Xm_a \\)          | Alice's partial  Monero public key on ed25519 |
| Bob's Monero key            | \\( Xm_b \\)          | Bob's partial  Monero public key on ed25519   |
| Script key                  | \\( K_s \\)           | The [script key] of the utxo |
| Alice's Script key          | \\( K_sa \\)          | Alice's partial [script key]  |
| Bob's Script key            | \\( K_sb \\)          | Bob's partial [script key]  |
| Alice's adaptor signature   | \\( b'_{Sa} \\)       | Alice's adaptor signature for the signature \\( b_{Sa} \\) of the script_signature of the utxo |
| Bob's adaptor signature     | \\( b'_{Sb} \\)       | Bob's adaptor signature for the \\( b_{Sb} \\) of the script_signature of the utxo |
| Alice's ZK tari proof       | \\(R_{ZTa}, s_{ZTa} \\)  | Zero knowledge proof signature for Alice's key \\(x_a) |
| Bob's ZK tari proof         | \\(R_{ZTb}, s_{ZTb})  | Zero knowledge proof signature for Bob's key \\(x_b) |
| Alice's ZK monero proof     | \\(R_{ZMa}, s_{ZMa})  | Zero knowledge proof signature for Alice's key \\(xm_a) |
| Bob's ZK monero proof       | \\(R_{ZMb}, s_{ZMb})  | Zero knowledge proof signature for Bob's key \\(xm_b) |
| Ristretto G generator       | \\(k \cdot G  \\)     | Value k over Tari G generator |
| Ristretto H generator       | \\(k \cdot H  \\)     | Value k over Tari H generator |
| ed25519 G generator         | \\(k \cdot M  \\)     | Value k over Monero G generator |


[HTLC]: Glossary.md#hashed-time-locked-contract
[Mempool]: Glossary.md#mempool
[Mimblewimble]: Glossary.md#mimblewimble
[TariScript]: Glossary.md#tariscript
[TariScript]: Glossary.md#tariscript
[script key]: Glossary.md#script-keypair
[sender offset key]: Glossary.md#sender-offset-keypair
[script offset]: Glossary.md#script-offset
