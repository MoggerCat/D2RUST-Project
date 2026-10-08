/* Stand-in for Game.exe in the cloud Wine pipeline (tools/cloud-game).
 *
 * Our own code, written for testing; nothing here is derived from Blizzard
 * code. A 32-bit GUI program at image base 0x400000 that:
 *   - prints its arguments to stdout and opens an 800x600 window (title
 *     "Diablo II", windowed like `-w`) painted with a fixed test pattern;
 *   - steps a seeded RNG of the shape described in specs/sim/rng.md §2
 *     through two non-inlined helpers (step, roll) and one inlined
 *     `mov ecx, K; mul ecx; add; adc` sequence, 25 times a second;
 *   - prints every left click and key it receives (autostart.py posts them);
 *     (`-busy`: no sleep, as fast as the debugger lets it run);
 *   - exits after N seconds (argument `-seconds N`, default 5)
 *     with exit code 7, printing the final seed.
 * tools/cloud-game/wine_probe.py points the trace recorder's Win32
 * debugger (tools/trace-recorder/record_rng.py) at the helper and the
 * inline site and checks the recording with check_rng.py.
 */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* seed: {lo, hi} */
static volatile unsigned int g_seed[2];

/* Helpers in plain asm so their entry bytes and calling convention are
 * fixed: ECX = seed pointer, EDX = n (roll). Exported names for nm. */
__asm__(
    ".text\n"
    ".globl _standin_rng_step\n"
    "_standin_rng_step:\n"          /* returns new lo */
    "  push ebx\n"
    "  mov ebx, ecx\n"
    "  mov eax, [ebx]\n"
    "  mov edx, 0x6AC690C5\n"
    "  mul edx\n"
    "  add eax, [ebx+4]\n"
    "  adc edx, 0\n"
    "  mov [ebx], eax\n"
    "  mov [ebx+4], edx\n"
    "  pop ebx\n"
    "  ret\n"
    ".globl _standin_rng_roll\n"
    "_standin_rng_roll:\n"          /* n < 1: 0, no draw; pow2: lo & (n-1); else lo % n */
    "  push esi\n"
    "  push ebx\n"
    "  mov esi, edx\n"
    "  test esi, esi\n"
    "  jle 1f\n"
    "  mov ebx, ecx\n"
    "  mov eax, [ebx]\n"
    "  mov edx, 0x6AC690C5\n"
    "  mul edx\n"
    "  add eax, [ebx+4]\n"
    "  adc edx, 0\n"
    "  mov [ebx], eax\n"
    "  mov [ebx+4], edx\n"
    "  lea edx, [esi-1]\n"
    "  test esi, edx\n"
    "  jnz 2f\n"
    "  and eax, edx\n"
    "  jmp 3f\n"
    "2:\n"
    "  xor edx, edx\n"
    "  div esi\n"
    "  mov eax, edx\n"
    "  jmp 3f\n"
    "1:\n"
    "  xor eax, eax\n"
    "3:\n"
    "  pop ebx\n"
    "  pop esi\n"
    "  ret\n"
    ".globl _standin_rng_set\n"
    "_standin_rng_set:\n"           /* seed setter: {EDX, [ESP+4]} */
    "  mov [ecx], edx\n"
    "  mov eax, [esp+4]\n"
    "  mov [ecx+4], eax\n"
    "  ret 4\n"
    ".globl _standin_rng_inline\n"
    "_standin_rng_inline:\n"        /* ECX = seed pointer; the inline shape */
    "  push esi\n"
    "  mov esi, ecx\n"
    "  mov eax, [esi]\n"
    "  mov ecx, 0x6AC690C5\n"
    "  mul ecx\n"
    "  add eax, [esi+4]\n"
    "  adc edx, 0\n"
    "  mov [esi], eax\n"
    "  mov [esi+4], edx\n"
    "  pop esi\n"
    "  ret\n");

unsigned int standin_rng_step(void);
unsigned int standin_rng_roll(void);
unsigned int standin_rng_set(void);
unsigned int standin_rng_inline(void);

static unsigned int call_step(volatile unsigned int *s)
{
    unsigned int r;
    __asm__ volatile("call _standin_rng_step" : "=a"(r) : "c"(s) : "edx", "memory", "cc");
    return r;
}
static unsigned int call_roll(volatile unsigned int *s, unsigned int n)
{
    unsigned int r, d;
    __asm__ volatile("call _standin_rng_roll" : "=a"(r), "=d"(d) : "c"(s), "d"(n) : "memory", "cc");
    return r;
}
static void call_set(volatile unsigned int *s, unsigned int lo, unsigned int hi)
{
    unsigned int d, c;
    __asm__ volatile("push %4\n\tcall _standin_rng_set"
                     : "=d"(d), "=c"(c) : "c"(s), "d"(lo), "r"(hi) : "eax", "memory", "cc");
}
static void call_inline(volatile unsigned int *s)
{
    unsigned int c;
    __asm__ volatile("call _standin_rng_inline" : "=c"(c) : "c"(s) : "eax", "edx", "memory", "cc");
}

static unsigned int g_frame;

static LRESULT CALLBACK wndproc(HWND h, UINT m, WPARAM w, LPARAM l)
{
    if (m == WM_PAINT) {
        PAINTSTRUCT ps;
        HDC dc = BeginPaint(h, &ps);
        /* fixed pattern: 8 vertical bars, a frame counter */
        static const COLORREF bars[8] = {RGB(255, 255, 255), RGB(255, 255, 0), RGB(0, 255, 255),
                                         RGB(0, 255, 0),     RGB(255, 0, 255), RGB(255, 0, 0),
                                         RGB(0, 0, 255),     RGB(0, 0, 0)};
        for (int i = 0; i < 8; i++) {
            RECT r = {i * 100, 0, i * 100 + 100, 500};
            HBRUSH b = CreateSolidBrush(bars[i]);
            FillRect(dc, &r, b);
            DeleteObject(b);
        }
        char txt[128];
        snprintf(txt, sizeof txt, "D2RUST stand-in  frame %u  seed %08X %08X", g_frame,
                 g_seed[0], g_seed[1]);
        TextOutA(dc, 20, 540, txt, (int)strlen(txt));
        EndPaint(h, &ps);
        return 0;
    }
    if (m == WM_LBUTTONDOWN || m == WM_KEYDOWN) {
        if (m == WM_LBUTTONDOWN)
            printf("standin: click %d %d\n", (int)(short)LOWORD(l), (int)(short)HIWORD(l));
        else
            printf("standin: key %u\n", (unsigned)w);
        fflush(stdout);
        return 0;
    }
    if (m == WM_DESTROY) {
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcA(h, m, w, l);
}

int main(int argc, char **argv)
{
    int seconds = 5, busy = 0;
    printf("standin: start, %d args:", argc - 1);
    for (int i = 1; i < argc; i++) {
        printf(" %s", argv[i]);
        if (!strcmp(argv[i], "-seconds") && i + 1 < argc)
            seconds = atoi(argv[i + 1]);
        if (!strcmp(argv[i], "-busy"))
            busy = 1;
    }
    printf("\n");
    fflush(stdout);

    WNDCLASSA wc = {0};
    wc.lpfnWndProc = wndproc;
    wc.hInstance = GetModuleHandleA(NULL);
    wc.lpszClassName = "Diablo II";
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    RegisterClassA(&wc);
    RECT r = {0, 0, 800, 600};
    AdjustWindowRect(&r, WS_OVERLAPPEDWINDOW, FALSE);
    HWND hwnd = CreateWindowA("Diablo II", "Diablo II", WS_OVERLAPPEDWINDOW | WS_VISIBLE, 0, 0,
                              r.right - r.left, r.bottom - r.top, NULL, NULL, wc.hInstance, NULL);
    printf("standin: window %p\n", (void *)hwnd);
    fflush(stdout);

    call_set(g_seed, 1234, 666);
    DWORD t0 = GetTickCount();
    unsigned int acc = 0;
    while (GetTickCount() - t0 < (DWORD)seconds * 1000) {
        MSG msg;
        while (PeekMessageA(&msg, NULL, 0, 0, PM_REMOVE)) {
            if (msg.message == WM_QUIT)
                goto done;
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
        g_frame++;
        acc += call_step(g_seed);
        acc += call_roll(g_seed, 100);
        acc += call_roll(g_seed, 16);
        acc += call_roll(g_seed, 0); /* no draw */
        call_inline(g_seed);
        InvalidateRect(hwnd, NULL, FALSE);
        if (!busy)
            Sleep(40);
    }
done:
    printf("standin: %u frames, seed %08X %08X, acc %08X\n", g_frame, g_seed[0], g_seed[1], acc);
    fflush(stdout);
    return 7;
}
