#!/usr/bin/env python3
"""Sign with RSA-PSS (SHA-256) through a PKCS#11 module, the way Firefox does
for TLS 1.3 client authentication (read-only session, one C_Sign).

usage: sign-test.py MODULE [TOKEN-LABEL-PREFIX] [rw] [len] [oslock] [iface] [reinit] [fixed|random ...]

Each fixed/random word is one signature: "fixed" signs the same 32 bytes
every time, "random" signs fresh random bytes. "rw" opens a read-write
session, "len" asks for the signature length first (as the cryptoki crate
does), "oslock" initializes the module with CKF_OS_LOCKING_OK (as p11-kit
does), "iface" takes the function list from C_GetInterface (PKCS#11 3.0,
which recent p11-kit prefers) instead of C_GetFunctionList, "reinit" does
C_Initialize, C_Finalize, C_Initialize first (as a p11-kit connection does).
Prints how long each signature took and what it is: the
signature is opened with the key's public modulus and checked as RSA-PSS
(SHA-256, MGF1-SHA256, salt 32) and as PKCS#1 v1.5. PIN=... logs in first (not needed
for SerproID). Uses only the standard library.
"""
import ctypes as C
import hashlib
import os
import sys
import time

CK_ULONG = C.c_ulong
CK_RV = CK_ULONG
CKF_SERIAL_SESSION = 4
CKA_CLASS = 0
CKO_PRIVATE_KEY = 3
CKM_RSA_PKCS_PSS = 0x0D
CKM_SHA256 = 0x250
CKG_MGF1_SHA256 = 2
FIXED = bytes.fromhex(
    "399b35de5597b9a1abcb611732a67bbf7949c46c71c253d69651340b5285d96e")


class Mechanism(C.Structure):
    _fields_ = [("mechanism", CK_ULONG), ("pParameter", C.c_void_p),
                ("ulParameterLen", CK_ULONG)]


class PssParams(C.Structure):
    _fields_ = [("hashAlg", CK_ULONG), ("mgf", CK_ULONG), ("sLen", CK_ULONG)]


class Attribute(C.Structure):
    _fields_ = [("type", CK_ULONG), ("pValue", C.c_void_p),
                ("ulValueLen", CK_ULONG)]


def mgf1(seed, length):
    out = b""
    for c in range((length + 31) // 32):
        out += hashlib.sha256(seed + c.to_bytes(4, "big")).digest()
    return out[:length]


def classify(sig, data, n, e):
    """What the signature is, opened with (n, e)."""
    k = (n.bit_length() + 7) // 8
    em = pow(int.from_bytes(sig, "big"), e, n).to_bytes(k, "big")
    # PKCS#1 v1.5: 00 01 FF..FF 00 payload
    if em[:2] == b"\x00\x01":
        payload = em[em.index(b"\x00", 2) + 1:]
        if payload == data:
            return "PKCS#1 v1.5 of the data as given (NOT PSS)"
        if payload.endswith(data):
            return f"PKCS#1 v1.5 with DigestInfo {payload[:-len(data)].hex()} (NOT PSS)"
        if payload.endswith(hashlib.sha256(data).digest()):
            return "PKCS#1 v1.5 of SHA-256(data), hashed again (NOT PSS)"
        return f"PKCS#1 v1.5 of something else: {payload.hex()[:80]}"
    # EMSA-PSS-VERIFY (RFC 8017 9.1.2), emBits = modBits - 1
    em_bits = n.bit_length() - 1
    em_len = (em_bits + 7) // 8
    em = em[-em_len:]
    if em[-1] != 0xBC:
        return f"neither PSS nor v1.5 (opens to {em[:8].hex()}…{em[-4:].hex()})"
    masked, h = em[: em_len - 33], em[em_len - 33: -1]
    db = bytes(a ^ b for a, b in zip(masked, mgf1(h, len(masked))))
    db = bytes([db[0] & (0xFF >> (8 * em_len - em_bits))]) + db[1:]
    salt = db[-32:]
    ok = db[: -33].strip(b"\x00") == b"" and db[-33] == 1
    ok = ok and hashlib.sha256(b"\x00" * 8 + data + salt).digest() == h
    return "valid RSA-PSS" if ok else "PSS-shaped but INVALID for this data"


class Interface(C.Structure):
    _fields_ = [("pInterfaceName", C.c_char_p), ("pFunctionList", C.c_void_p),
                ("flags", CK_ULONG)]


class InitArgs(C.Structure):
    _fields_ = [("CreateMutex", C.c_void_p), ("DestroyMutex", C.c_void_p),
                ("LockMutex", C.c_void_p), ("UnlockMutex", C.c_void_p),
                ("flags", CK_ULONG), ("pReserved", C.c_void_p)]


class TokenInfo(C.Structure):
    _fields_ = [("label", C.c_char * 32), ("rest", C.c_ubyte * 256)]


# Index of each function in CK_FUNCTION_LIST (after CK_VERSION).
IDX = dict(C_Initialize=0, C_Finalize=1, C_GetSlotList=4, C_GetTokenInfo=6,
           C_OpenSession=12, C_CloseSession=13, C_Login=18, C_GetAttributeValue=24,
           C_FindObjectsInit=26,
           C_FindObjects=27, C_FindObjectsFinal=28, C_SignInit=42, C_Sign=43)


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    lib = C.CDLL(sys.argv[1])
    prefix = "SerproID"
    runs = []
    rw = "rw" in sys.argv[2:]
    ask_len = "len" in sys.argv[2:]
    oslock = "oslock" in sys.argv[2:]
    iface = "iface" in sys.argv[2:]
    reinit = "reinit" in sys.argv[2:]
    for a in sys.argv[2:]:
        if a in ("rw", "len", "oslock", "iface", "reinit"):
            continue
        if a in ("fixed", "random"):
            runs.append(a)
        elif a.startswith("-") or "/" in a or ":" in a:
            continue  # ignored (lets a test mimic another command line)
        else:
            prefix = a
    runs = runs or ["random"]

    has_iface = hasattr(lib, "C_GetInterface")
    print(f"module exports C_GetInterface: {'yes' if has_iface else 'no'}")
    flist = C.c_void_p()
    if iface:
        if not has_iface:
            raise SystemExit("no C_GetInterface")
        ip = C.POINTER(Interface)()
        rv = lib.C_GetInterface(None, None, C.byref(ip), CK_ULONG(0))
        assert rv == 0, f"C_GetInterface: {rv:#x}"
        flist = C.c_void_p(ip.contents.pFunctionList)
    else:
        rv = lib.C_GetFunctionList(C.byref(flist))
        assert rv == 0, f"C_GetFunctionList: {rv:#x}"
    ver = C.cast(flist, C.POINTER(C.c_ubyte * 2)).contents
    print(f"function list: {'C_GetInterface' if iface else 'C_GetFunctionList'}, "
          f"version {ver[0]}.{ver[1]}")
    table = C.cast(flist.value + 8, C.POINTER(C.c_void_p))

    def f(name, *argtypes):
        return C.CFUNCTYPE(CK_RV, *argtypes)(table[IDX[name]])

    def check(name, rv):
        if rv != 0:
            raise SystemExit(f"{name}: CKR {rv:#x}")

    P = C.c_void_p
    PU = C.POINTER(CK_ULONG)
    init_args = InitArgs(None, None, None, None, 2, None)  # CKF_OS_LOCKING_OK
    check("C_Initialize", f("C_Initialize", P)(
        C.byref(init_args) if oslock else None))
    print(f"init: {'CKF_OS_LOCKING_OK' if oslock else 'NULL'}")
    if reinit:
        n0 = CK_ULONG()
        f("C_GetSlotList", C.c_ubyte, C.POINTER(CK_ULONG), C.POINTER(CK_ULONG))(
            1, None, C.byref(n0))
        check("C_Finalize", f("C_Finalize", P)(None))
        check("C_Initialize", f("C_Initialize", P)(
            C.byref(init_args) if oslock else None))
        print("init: finalized and initialized again")
    n = CK_ULONG()
    get_slots = f("C_GetSlotList", C.c_ubyte, PU, PU)
    check("C_GetSlotList", get_slots(1, None, C.byref(n)))
    slots = (CK_ULONG * n.value)()
    check("C_GetSlotList", get_slots(1, slots, C.byref(n)))
    slot = None
    for s in slots[: n.value]:
        info = TokenInfo()
        f("C_GetTokenInfo", CK_ULONG, P)(s, C.byref(info))
        label = info.label.decode("latin-1").strip(" \0")
        print(f"token: {label!r}")
        if label.startswith(prefix) and slot is None:
            slot = s
    if slot is None:
        raise SystemExit(f"no token whose label starts with {prefix!r}")

    sess = CK_ULONG()
    check("C_OpenSession", f("C_OpenSession", CK_ULONG, CK_ULONG, P, P, PU)(
        slot, CKF_SERIAL_SESSION | (2 if rw else 0), None, None, C.byref(sess)))
    pin = os.environ.get("PIN", "").encode()
    if pin:
        check("C_Login", f("C_Login", CK_ULONG, CK_ULONG, C.c_char_p, CK_ULONG)(
            sess, 1, pin, len(pin)))
    cls = CK_ULONG(CKO_PRIVATE_KEY)
    tmpl = Attribute(CKA_CLASS, C.cast(C.byref(cls), P), C.sizeof(cls))
    check("C_FindObjectsInit", f("C_FindObjectsInit", CK_ULONG, P, CK_ULONG)(
        sess, C.byref(tmpl), 1))
    key = CK_ULONG()
    count = CK_ULONG()
    check("C_FindObjects", f("C_FindObjects", CK_ULONG, PU, CK_ULONG, PU)(
        sess, C.byref(key), 1, C.byref(count)))
    f("C_FindObjectsFinal", CK_ULONG)(sess)
    if count.value == 0:
        raise SystemExit("no private key")

    get_attr = f("C_GetAttributeValue", CK_ULONG, CK_ULONG, P, CK_ULONG)

    def attr(obj, kind):
        a = Attribute(kind, None, 0)
        if get_attr(sess, obj, C.byref(a), 1) != 0 or a.ulValueLen > 4096:
            return None
        buf = (C.c_ubyte * a.ulValueLen)()
        a.pValue = C.cast(buf, P)
        if get_attr(sess, obj, C.byref(a), 1) != 0:
            return None
        return int.from_bytes(bytes(buf), "big")

    modulus = attr(key.value, 0x120)  # CKA_MODULUS
    exponent = attr(key.value, 0x122) or 65537  # CKA_PUBLIC_EXPONENT
    if modulus:
        print(f"key: RSA {modulus.bit_length()} bits, e={exponent}")
    else:
        print("key: modulus not readable, signatures won't be checked")

    params = PssParams(CKM_SHA256, CKG_MGF1_SHA256, 32)
    mech = Mechanism(CKM_RSA_PKCS_PSS, C.cast(C.byref(params), P),
                     C.sizeof(params))
    sign_init = f("C_SignInit", CK_ULONG, P, CK_ULONG)
    sign = f("C_Sign", CK_ULONG, P, CK_ULONG, P, PU)
    status = 0
    print(f"session: {'read-write' if rw else 'read-only'}, "
          f"length query: {'yes' if ask_len else 'no'}")
    for i, kind in enumerate(runs, 1):
        data = FIXED if kind == "fixed" else os.urandom(32)
        out = (C.c_ubyte * 512)()
        out_len = CK_ULONG(512)
        print(f"[{i}] {kind}: signing — approve on the phone if asked...",
              flush=True)
        t = time.monotonic()
        rv = sign_init(sess, C.byref(mech), key)
        if rv == 0 and ask_len:
            rv = sign(sess, data, len(data), None, C.byref(out_len))
        if rv == 0:
            rv = sign(sess, data, len(data), out, C.byref(out_len))
        dt = time.monotonic() - t
        if rv == 0:
            sig = bytes(out[: out_len.value])
            what = classify(sig, data, modulus, exponent) if modulus else "?"
            print(f"[{i}] {kind}: OK in {dt:.1f}s, signature {sig[:8].hex()}…: {what}")
        else:
            print(f"[{i}] {kind}: CKR {rv:#x} after {dt:.1f}s")
            status = 1
    f("C_CloseSession", CK_ULONG)(sess)
    f("C_Finalize", P)(None)
    return status


if __name__ == "__main__":
    sys.exit(main())
