"use client";
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { syncRegister, syncSignIn, syncStatus, type ErrorCategory } from "../../services/tauri/sync";
import { CATEGORY_MESSAGES } from "../sync/SyncPanel";
import { SignInForm } from "./SignInForm";

// After a create, the sign-in step is what refuses: an existing email gets the same 201, then a wrong-password answer.
const REGISTER_REFUSED_SIGNIN =
  "This email could not be signed in with that password. If you already have an account, use Sign in.";

// Remembered on this device: after "Continue without an account" the gate stays hidden on later launches. Signing in
// from the Accounts page still works at any time.
const SKIP_KEY = "lunorx.loginSkipped";

function readSkipped(): boolean {
  try {
    return window.localStorage.getItem(SKIP_KEY) === "1";
  } catch {
    return false;
  }
}

function writeSkipped(value: boolean) {
  try {
    if (value) window.localStorage.setItem(SKIP_KEY, "1");
    else window.localStorage.removeItem(SKIP_KEY);
  } catch {
    /* storage unavailable: the gate simply shows again next launch */
  }
}

// Rejections are bare category strings; anything else falls back to the generic category.
function toCategory(e: unknown): ErrorCategory {
  return typeof e === "string" && Object.hasOwn(CATEGORY_MESSAGES, e) ? (e as ErrorCategory) : "internal";
}

/** Full-window sign-in shown at launch while the LunoRX account is signed out. Skippable. */
export function LoginGate() {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<"signIn" | "register">("signIn");

  useEffect(() => {
    let active = true;
    if (readSkipped()) return;
    syncStatus().then(
      (s) => { if (active && s.state.state === "signedOut") setOpen(true); },
      () => { /* sync unavailable: nothing to sign in to, so no gate */ },
    );
    return () => { active = false; };
  }, []);

  if (!open) return null;

  async function submit(email: string, secret: string, antibot: string[]) {
    setBusy(true);
    setError(null);
    try {
      if (mode === "register") await syncRegister(email, secret, antibot);
      else await syncSignIn(email, secret, antibot[0]);
      setOpen(false);
    } catch (err) {
      const category = toCategory(err);
      setError(
        mode === "register" && category === "unauthenticated" ? REGISTER_REFUSED_SIGNIN : CATEGORY_MESSAGES[category],
      );
    }
    setBusy(false);
  }

  function continueWithoutAccount() {
    writeSkipped(true);
    setOpen(false);
  }

  return (
    <div className="fixed inset-0 z-[90] flex items-center justify-center overflow-y-auto bg-night-canvas p-6">
      <section role="dialog" aria-modal="true" aria-labelledby="login-gate-title" className="glass-solid flex w-full max-w-[420px] flex-col gap-6 rounded-[16px] p-8">
        <header className="flex flex-col gap-2">
          <h1 id="login-gate-title" className="font-display text-[28px] leading-9 font-bold tracking-[-0.02em]">Sign in to LunoRX</h1>
          <p className="text-[14px] leading-[20px] text-secondary-text">
            Sync your playlists, likes and listening stats across your devices. You can also use LunoRX without an account.
          </p>
        </header>
        <SignInForm
          idPrefix="login-gate"
          mode={mode}
          busy={busy}
          error={error}
          onSubmit={submit}
        />
        <div className="flex flex-col items-center gap-1">
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => { setError(null); setMode(mode === "signIn" ? "register" : "signIn"); }}
          >
            {mode === "signIn" ? "Create an account" : "I already have an account"}
          </Button>
          <Button variant="ghost" disabled={busy} onClick={continueWithoutAccount}>Continue without an account</Button>
        </div>
      </section>
    </div>
  );
}
