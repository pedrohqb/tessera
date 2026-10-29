#!/usr/bin/env python3
"""Sign with RSA-PSS (SHA-256) through a PKCS#11 module, the way Firefox does
for TLS 1.3 client authentication (read-only session, one C_Sign).

usage: sign-test.py MODULE [TOKEN-LABEL-PREFIX] [rw] [len] [fixed|random ...]

Each fixed/random word is one signature: "fixed" signs the same 32 bytes
every time, "random" signs fresh random bytes. "rw" opens a read-write
session, "len" asks for the signature length first (as the cryptoki crate
does). Prints the first bytes of
each signature and how long it took. PIN=... logs in first (not needed
for SerproID). Uses only the standard library.
"""
import ctypes as C
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


class TokenInfo(C.Structure):
    _fields_ = [("label", C.c_char * 32), ("rest", C.c_ubyte * 256)]


# Index of each function in CK_FUNCTION_LIST (after CK_VERSION).
IDX = dict(C_Initialize=0, C_Finalize=1, C_GetSlotList=4, C_GetTokenInfo=6,
           C_OpenSession=12, C_CloseSession=13, C_Login=18, C_FindObjectsInit=26,
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
    for a in sys.argv[2:]:
        if a in ("rw", "len"):
            continue
        if a in ("fixed", "random"):
            runs.append(a)
        else:
            prefix = a
    runs = runs or ["random"]

    flist = C.c_void_p()
    rv = lib.C_GetFunctionList(C.byref(flist))
    assert rv == 0, f"C_GetFunctionList: {rv:#x}"
    table = C.cast(flist.value + 8, C.POINTER(C.c_void_p))

    def f(name, *argtypes):
        return C.CFUNCTYPE(CK_RV, *argtypes)(table[IDX[name]])

    def check(name, rv):
        if rv != 0:
            raise SystemExit(f"{name}: CKR {rv:#x}")

    P = C.c_void_p
    PU = C.POINTER(CK_ULONG)
    check("C_Initialize", f("C_Initialize", P)(None))
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
            print(f"[{i}] {kind}: OK in {dt:.1f}s, signature {sig[:8].hex()}…")
        else:
            print(f"[{i}] {kind}: CKR {rv:#x} after {dt:.1f}s")
            status = 1
    f("C_CloseSession", CK_ULONG)(sess)
    f("C_Finalize", P)(None)
    return status


if __name__ == "__main__":
    sys.exit(main())
