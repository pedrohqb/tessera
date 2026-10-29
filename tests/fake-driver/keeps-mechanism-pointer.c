/* A PKCS#11 module that, like SerproID, keeps the CK_MECHANISM pointer from
 * C_SignInit and reads the PSS parameters only in C_Sign. */
#include <string.h>
#include <stdio.h>
#define CK_PTR *
#define CK_DECLARE_FUNCTION(r, n) r n
#define CK_DECLARE_FUNCTION_POINTER(r, n) r (* n)
#define CK_CALLBACK_FUNCTION(r, n) r (* n)
#define NULL_PTR 0
#include <p11-kit-1/p11-kit/pkcs11.h>

static CK_MECHANISM_PTR saved;
static CK_FUNCTION_LIST list;
static CK_RV stub(void) { return CKR_FUNCTION_NOT_SUPPORTED; }
static CK_RV init(CK_VOID_PTR a) { return CKR_OK; }
static CK_RV fin(CK_VOID_PTR a) { return CKR_OK; }
static CK_RV info(CK_INFO_PTR i) { memset(i, ' ', sizeof *i); i->cryptokiVersion.major = 2; i->cryptokiVersion.minor = 20; i->flags = 0; return CKR_OK; }
static CK_RV slots(CK_BBOOL p, CK_SLOT_ID_PTR l, CK_ULONG_PTR n) { if (l) { if (*n < 1) { *n = 1; return CKR_BUFFER_TOO_SMALL; } l[0] = 1; } *n = 1; return CKR_OK; }
static CK_RV slotinfo(CK_SLOT_ID s, CK_SLOT_INFO_PTR i) { memset(i, ' ', sizeof *i); i->flags = CKF_TOKEN_PRESENT; i->hardwareVersion.major = 1; i->firmwareVersion.major = 1; i->hardwareVersion.minor = 0; i->firmwareVersion.minor = 0; return CKR_OK; }
static CK_RV tokeninfo(CK_SLOT_ID s, CK_TOKEN_INFO_PTR i) { memset(i, ' ', sizeof *i); memcpy(i->label, "Fake", 4); i->flags = CKF_TOKEN_INITIALIZED; i->ulMaxSessionCount = i->ulSessionCount = i->ulMaxRwSessionCount = i->ulRwSessionCount = 0; i->ulMaxPinLen = 8; i->ulMinPinLen = 4; i->ulTotalPublicMemory = i->ulFreePublicMemory = i->ulTotalPrivateMemory = i->ulFreePrivateMemory = (CK_ULONG)-1; memset(&i->hardwareVersion, 0, 4); return CKR_OK; }
static CK_RV mechs(CK_SLOT_ID s, CK_MECHANISM_TYPE_PTR l, CK_ULONG_PTR n) { if (l) l[0] = CKM_RSA_PKCS_PSS; *n = 1; return CKR_OK; }
static CK_RV open_(CK_SLOT_ID s, CK_FLAGS f, CK_VOID_PTR a, CK_NOTIFY no, CK_SESSION_HANDLE_PTR h) { *h = 7; return CKR_OK; }
static CK_RV close_(CK_SESSION_HANDLE h) { return CKR_OK; }
static CK_RV closeall(CK_SLOT_ID s) { return CKR_OK; }
static CK_RV sessinfo(CK_SESSION_HANDLE h, CK_SESSION_INFO_PTR i) { i->slotID = 1; i->state = CKS_RO_PUBLIC_SESSION; i->flags = CKF_SERIAL_SESSION; i->ulDeviceError = 0; return CKR_OK; }
static int found;
static CK_RV findinit(CK_SESSION_HANDLE h, CK_ATTRIBUTE_PTR t, CK_ULONG n) { found = 0; return CKR_OK; }
static CK_RV find(CK_SESSION_HANDLE h, CK_OBJECT_HANDLE_PTR o, CK_ULONG m, CK_ULONG_PTR n) { if (!found && m) { o[0] = 42; *n = 1; found = 1; } else *n = 0; return CKR_OK; }
static CK_RV findfin(CK_SESSION_HANDLE h) { return CKR_OK; }
static CK_RV attr(CK_SESSION_HANDLE h, CK_OBJECT_HANDLE o, CK_ATTRIBUTE_PTR t, CK_ULONG n) { for (CK_ULONG k = 0; k < n; k++) t[k].ulValueLen = (CK_ULONG)-1; return CKR_ATTRIBUTE_TYPE_INVALID; }
static CK_RV signinit(CK_SESSION_HANDLE h, CK_MECHANISM_PTR m, CK_OBJECT_HANDLE k) { saved = m; return CKR_OK; }
static CK_RV sign(CK_SESSION_HANDLE h, CK_BYTE_PTR d, CK_ULONG dl, CK_BYTE_PTR s, CK_ULONG_PTR sl) {
    CK_RSA_PKCS_PSS_PARAMS *p = saved->pParameter;
    int ok = saved->mechanism == CKM_RSA_PKCS_PSS && saved->ulParameterLen == sizeof *p &&
             p->hashAlg == CKM_SHA256 && p->mgf == CKG_MGF1_SHA256 && p->sLen == 32;
    fprintf(stderr, "fake: C_Sign sees mech=%#lx hash=%#lx mgf=%#lx sLen=%lu -> %s\n",
            saved->mechanism, p ? p->hashAlg : 0, p ? p->mgf : 0, p ? p->sLen : 0, ok ? "OK" : "WRONG");
    if (!ok) return CKR_FUNCTION_FAILED;
    if (!s) { *sl = 4; return CKR_OK; }
    memcpy(s, "good", 4); *sl = 4; return CKR_OK;
}
CK_RV C_GetFunctionList(CK_FUNCTION_LIST_PTR_PTR l) {
    void **f = (void **)&list.C_Initialize;
    for (unsigned i = 0; i < 68; i++) f[i] = (void *)stub;
    list.version.major = 2; list.version.minor = 20;
    list.C_Initialize = init; list.C_Finalize = fin; list.C_GetInfo = info; list.C_GetFunctionList = C_GetFunctionList;
    list.C_GetSlotList = slots; list.C_GetSlotInfo = slotinfo; list.C_GetTokenInfo = tokeninfo;
    list.C_GetMechanismList = mechs; list.C_OpenSession = open_; list.C_CloseSession = close_;
    list.C_CloseAllSessions = closeall; list.C_GetSessionInfo = sessinfo;
    list.C_FindObjectsInit = findinit; list.C_FindObjects = find; list.C_FindObjectsFinal = findfin;
    list.C_GetAttributeValue = attr; list.C_SignInit = signinit; list.C_Sign = sign;
    *l = &list; return CKR_OK;
}
