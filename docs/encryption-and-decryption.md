# Encryption And Decryption

This document has two audiences. The first half is a **plain-English primer** for anyone — including people who have never studied cryptography — so you can understand *what* the app protects and *why* it uses "quantum-safe" cryptography. The second half is the **precise technical reference** that auditors and engineers use to verify the exact paths in the codebase.

If you only want the deep technical detail, jump to [For Reviewers: Four Questions This Doc Answers](#for-reviewers-four-questions-this-doc-answers).

---

## Start Here: The Whole Idea In One Picture

Imagine you want to put a document in a safe so that only certain people can ever open it.

1. You lock the **document** inside a strong box with a single key. (This is fast and works for files of any size.)
2. You then make a **copy of that one key for each authorized person**, and lock each copy inside a tiny personal lockbox that only that specific person can open.
3. You store the strong box and all the tiny lockboxes together.

To read the document, an authorized person opens *their* tiny lockbox, takes out the key copy, and uses it to open the big box. Nobody else's lockbox helps them — and the big box itself is useless without a key.

That is exactly how this app works. The "strong box" is the encrypted file, the "single key" is called the **Content Encryption Key (CEK)**, and the "tiny personal lockboxes" are created using **post-quantum cryptography**. This pattern has a name: **envelope encryption**.

This two-layer design is why the system can share one document with many people *without* re-encrypting the whole file for each person — it just adds one more tiny lockbox.

## The Three Building Blocks (In Plain Words)

Cryptography here is built from three different tools, each doing a job the others can't:

| Tool in this app | Job | Everyday analogy |
| --- | --- | --- |
| **XChaCha20-Poly1305** (a *symmetric cipher*) | Scrambles the actual document bytes with one secret key. Fast, handles large files. | The lock on the big strong box. |
| **ML-KEM** (a *key encapsulation mechanism*) | Lets someone lock a secret *for* you using only your **public** key, so only your **private** key can unlock it. | Each person's tiny personal lockbox. |
| **ML-DSA** (a *digital signature*) | Lets someone prove "I, the holder of this key, approved this exact document" — and lets anyone verify it. | A tamper-evident wax seal / signature. |

"Symmetric" means the *same* secret key locks and unlocks (good for speed, bad for sharing — how do you give someone the key safely?). "Public-key" tools like ML-KEM and ML-DSA solve the sharing problem: you can hand out a public key to the whole world, and it only ever lets people *lock things for you* or *check your signatures* — never impersonate you or read your mail.

Two supporting helpers also appear:
- **HKDF-SHA256** turns the raw shared secret produced by ML-KEM into a clean, fixed-size wrapping key. (Think: standardizing a rough key blank into one that fits the lock precisely.)
- **SHA3-256** produces a "fingerprint" (hash) of the original file — a short, unique value used as custody evidence to prove a file hasn't changed.

## What "Quantum-Safe" Means And Why This Project Cares

Most encryption on the internet today (RSA, elliptic-curve / ECC) is secure only because certain math problems are too slow for normal computers to solve. A large enough **quantum computer** running **Shor's algorithm** would solve exactly those problems quickly — breaking RSA and ECC.

That matters *today*, even though big quantum computers don't exist yet, because of **"harvest now, decrypt later"**: an attacker can copy encrypted data now and simply wait. If the data was protected only by RSA/ECC, a future quantum computer could decrypt the stored copy years later. For documents that must stay confidential for a long time, that is a real risk.

The defense is **post-quantum cryptography (PQC)**: algorithms whose underlying math is believed to be hard *even for quantum computers*. In 2024 the U.S. standards body **NIST** finalized the first PQC standards, and this project uses two of them:

- **ML-KEM** — standardized as **FIPS 203** (formerly known as "Kyber"). Replaces the key-exchange/key-wrapping job that RSA/ECC used to do. This app uses **ML-KEM-768** for documents.
- **ML-DSA** — standardized as **FIPS 204** (formerly known as "Dilithium"). Replaces RSA/ECC digital signatures. This app uses **ML-DSA-65**.

A common question: *aren't symmetric ciphers also at risk?* Quantum computers (via Grover's algorithm) only **halve** the effective strength of a symmetric cipher. A 256-bit key like the one XChaCha20-Poly1305 uses still leaves ~128 bits of post-quantum security, which is considered safe. So only the **public-key** parts needed replacing — which is precisely what ML-KEM and ML-DSA do.

## A Walk-Through, No Jargon

**Saving (encrypting) a document:**
1. A random one-time key (the CEK) is generated.
2. The document is locked with that key using XChaCha20-Poly1305 → this is the big strong box.
3. For each authorized recipient, ML-KEM uses *their public key* to produce a shared secret; HKDF turns it into a wrapping key; that wrapping key locks a copy of the CEK → one tiny lockbox per person.
4. The strong box plus all the lockboxes are bundled into a single signed JSON file called the **envelope**, and stored.

**Opening (decrypting) a document:**
1. The system finds *your* lockbox inside the envelope.
2. ML-KEM uses your **private** key to recover the same shared secret; HKDF rebuilds the wrapping key; that opens your lockbox and reveals the CEK.
3. The CEK opens the big strong box, revealing the original document — but only after access-control checks pass.

Both the browser and the backend use the **same** ML-KEM-768 implementation (the pure-Rust `fips203` crate), so a document encrypted in the browser decrypts on the server and vice-versa. The rest of this document describes exactly *where* each step runs (browser vs. backend), *who* holds which keys, and *what* gets recorded as evidence.

---

## For Reviewers: Four Questions This Doc Answers

The remainder is the precise reference for the encryption, decryption, signing, and anchoring paths that exist in the current codebase. It answers:

1. Where is plaintext created and where is it exposed?
2. What is encrypted in the browser versus on the backend?
3. Who can decrypt a stored document?
4. What is application evidence versus what is anchored to Arweave?

## Current Storage Modes

The app currently stores documents as canonical PQ envelope objects in Supabase Storage.

There are now two supported envelope creation paths:

### `pq_envelope_server_managed`

Legacy and compatibility path.

- Browser uploads plaintext bytes to `/api/doc/upload` or `/api/doc/:id/version`
- Backend creates `DocumentEnvelopeV1`
- Backend encrypts payload with a random CEK using `XChaCha20-Poly1305`
- Backend wraps the CEK for the owner using server-held ML-KEM-768 keys
- Backend stores the canonical envelope JSON in Supabase Storage

### `pq_envelope_browser_encrypted`

Current web upload path.

- Browser reads the file bytes locally
- Browser generates a random CEK
- Browser encrypts the payload locally with `XChaCha20-Poly1305`
- Browser encapsulates to the active wallet's server-managed ML-KEM public key
- Browser derives the CEK wrap key with `HKDF-SHA256`
- Browser wraps the CEK locally with `XChaCha20-Poly1305`
- Browser uploads the canonical envelope JSON, not plaintext
- Backend validates the envelope, assigns the real document id into metadata, and stores it

The browser path is implemented through:

- `backend-rs/web/app.js`
- `backend-rs/web/pq-worker.js`
- `backend-rs/pq-wasm/src/lib.rs`

The backend validation and storage path is implemented through:

- `backend-rs/src/main.rs`
- `backend-rs/src/crypto/canonical/envelope.rs`

## Envelope Format

Stored envelopes use `DocumentEnvelopeV1`.

Important fields:

- `v`: envelope version
- `owner`: normalized wallet id for the owner
- `created_at`: envelope creation timestamp
- `doc.logical_id`: logical document id
- `doc.filename`: custody label / display label
- `doc.mime`: original document mime type
- `doc.plaintext_sha3_256_hex`: custody hash of plaintext
- `doc.size_bytes`: plaintext byte length
- `encryption.alg`: payload cipher, currently `xchacha20poly1305`
- `encryption.cek_wrap`: CEK protection mode, currently `mlkem`
- `encryption.wrapped_keys[*]`: per-recipient ML-KEM ciphertext plus wrapped CEK
- `ciphertext_b64`: encrypted payload bytes

The current browser upload path uses one wrapped recipient key: the active owner wallet's server-managed ML-KEM key.

The schema is already multi-recipient capable, which is the main foundation for future org-managed custody and recovery.

## Browser Encryption Flow

For browser uploads, versions, and browser text-editor saves:

1. Browser loads the active wallet session.
2. Browser fetches `mlkem_pk_b64` from `/auth/session`.
3. Browser computes the plaintext SHA3-256 custody hash.
4. Browser generates:
   - a 32-byte CEK
   - a 24-byte payload nonce
   - a 24-byte CEK-wrap nonce
   - a 32-byte ML-KEM encapsulation seed
5. Browser encrypts plaintext with `XChaCha20-Poly1305`.
6. Browser performs ML-KEM-768 encapsulation to the owner public key.
7. Browser derives a wrap key with `HKDF-SHA256` using info string `tidbit-cek-wrap-v1`.
8. Browser wraps the CEK with `XChaCha20-Poly1305`.
9. Browser serializes canonical `DocumentEnvelopeV1` JSON.
10. Browser uploads the envelope blob with `encryption_source=browser_pq_envelope_v1`.

Important trust note:

- plaintext is not sent to the backend on this path
- this is browser-side encryption
- it is not full end-to-end user-held decryption because the owner ML-KEM secret key is still server-managed

## Backend Decryption Flow

When an owner or authorized recipient downloads or reviews a document:

1. Backend loads the stored envelope bytes from Supabase Storage.
2. Backend parses `DocumentEnvelopeV1`.
3. Backend loads the owner wallet's ML-KEM secret key from `wallet_mlkem_keys`.
4. Backend selects the wrapped key entry for the owner wallet.
5. Backend decapsulates the ML-KEM ciphertext to recover the shared secret.
6. Backend derives the wrap key with `HKDF-SHA256`.
7. Backend decrypts the wrapped CEK with `XChaCha20-Poly1305`.
8. Backend decrypts the payload ciphertext with `XChaCha20-Poly1305`.
9. Backend serves plaintext bytes only after access control passes.

This is why the current system should be described as:

- browser-side encrypted upload
- server-managed owner custody
- backend-enforced access-controlled decryption

## ML-KEM And ML-DSA Roles

These are separate:

### ML-KEM

Used for document envelope key wrapping.

- current storage path: ML-KEM-768 (FIPS 203)
- implementation: pure-Rust `fips203` crate on both the browser (wasm) and backend paths
- purpose: protect the CEK used for document payload encryption
- browser path: encapsulation in wasm
- backend path: decapsulation on download/review

### ML-DSA

Used for signatures and attestations.

- current signing path: ML-DSA-65 via `fips204`
- browser path: device-local key generation, backup/import, sign, and verify
- backend path: verify signature proof and write custody evidence

Browser-local ML-DSA keys are independent from document decryption keys.

## Share Anchoring On Arweave

Share activity is primarily application evidence inside:

- `document_shares`
- `document_events`
- `growth_events`

The app can now also anchor share issuance records to Arweave.

What is anchored:

- a SHA3-256 hash of the share issuance record
- document id and document hash context
- sender wallet / chain
- recipient routing fields
- envelope id
- expiry and one-time/download/guest-sign settings

What is stored in the app:

- `share_anchor_hash_hex`
- `share_arweave_tx`
- `share_anchored_at`

Important boundary:

- the Arweave anchor represents share issuance evidence
- mutable follow-up activity like open, download, revoke, and completion still lives in the application custody ledger

## Crypto Agility Status

The code now has practical crypto-agility groundwork, but not a full algorithm-negotiation system yet.

Already present:

- envelope version field `v`
- explicit payload algorithm field `encryption.alg`
- explicit CEK wrap field `encryption.cek_wrap`
- per-recipient wrapped-key entries with `kem`
- separate browser and backend envelope creation paths using the same stored format

Still not finished:

- multi-recipient org custody rollout in the web app
- automatic recipient key fan-out for org recovery/admins
- migration tooling between envelope versions and algorithm sets
- user-held decryption without server-managed owner custody

## Reviewer Summary

If you are reviewing the current implementation, the precise statement is:

- browser uploads are now encrypted in the browser before they are sent
- stored objects remain canonical PQ envelopes
- decryption still depends on server-managed owner ML-KEM secret keys
- browser-local ML-DSA signing is separate and remains device-local
- Arweave anchoring is optional for both document evidence and share issuance evidence
