"use client";

import { useEffect, useState } from "react";
import { Cloud, Link2, ListMusic } from "lucide-react";
import { Button, Screen, Segmented } from "../../components/ui";
import { useUi } from "../../lib/store";
import { syncRegister, syncSignIn, syncStatus, type ErrorCategory } from "../../services/tauri/sync";
import { Banner, PageRoot } from "../settings/rows";
import { CATEGORY_MESSAGES } from "../sync/SyncPanel";
import { SignInForm } from "./SignInForm";

// Its own page, so signing in is a step the user chooses and not a form inside Settings. Playlists work without it;
// uploading and sharing need it. A successful sign-in returns to the screen the user came from.
function toCategory(e: unknown): ErrorCategory {
  return typeof e === "string" && Object.hasOwn(CATEGORY_MESSAGES, e) ? (e as ErrorCategory) : "internal";
}

export default function LoginPage() {
  const navigate = useUi((s) => s.navigate);
  const goBack = useUi((s) => s.goBack);
  const hasBack = useUi((s) => s.back.length > 0);
  const [mode, setMode] = useState<"signIn" | "register">("signIn");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(0);

  useEffect(() => {
    syncStatus()
      .then((s) => setPending(s.pendingWrites))
      .catch(() => setPending(0));
  }, []);

  function leave() {
    if (hasBack) goBack();
    else navigate("accounts");
  }

  async function submit(email: string, secret: string, antibot: string[]) {
    setBusy(true);
    setError(null);
    try {
      if (mode === "register") await syncRegister(email, secret, antibot);
      else await syncSignIn(email, secret, antibot[0]);
      leave();
    } catch (err) {
      setError(CATEGORY_MESSAGES[toCategory(err)]);
    }
    setBusy(false);
  }

  const blocked =
    pending > 0 ? `Upload or discard the ${pending} unsent ${pending === 1 ? "change" : "changes"} in Sync first.` : undefined;

  return (
    <PageRoot>
      <Screen id="login-title" title={mode === "register" ? "Create account" : "Sign in"}>
        <div className="flex max-w-[640px] flex-col gap-6">
          <header className="flex items-center gap-4 rounded-[14px] bg-frost-100 p-5">
            <span aria-hidden="true" className="grid size-14 shrink-0 place-items-center rounded-[14px] bg-frost-300 text-primary-text">
              <Cloud size={26} strokeWidth={1.75} />
            </span>
            <div className="flex min-w-0 flex-col gap-1">
              <p className="text-[15px] leading-[20px] font-semibold text-primary-text">One LunoRX account for every device</p>
              <p className="text-[13px] leading-[18px] text-secondary-text">
                Playlists stay on this device without one. An account adds sync and sharing.
              </p>
            </div>
          </header>

          <ul aria-label="What an account adds" className="grid gap-2 sm:grid-cols-3">
            {[
              { Icon: ListMusic, text: "Sync playlists across devices" },
              { Icon: Link2, text: "Share a playlist with a link" },
              { Icon: Cloud, text: "Back up play counts to the cloud" },
            ].map(({ Icon, text }) => (
              <li key={text} className="flex items-start gap-2.5 rounded-[12px] bg-frost-100 p-3.5 text-[13px] leading-[18px] text-secondary-text">
                <Icon size={16} strokeWidth={1.75} aria-hidden="true" className="mt-px shrink-0 text-primary-text" />
                <span>{text}</span>
              </li>
            ))}
          </ul>

          <Segmented
            label="Account action"
            value={mode}
            options={[
              { key: "signIn", label: "I have an account" },
              { key: "register", label: "Create an account" },
            ]}
            onChange={(next) => { setMode(next); setError(null); }}
          />

          {error && <Banner tone="warn" role="alert">{error}</Banner>}

          <SignInForm
            key={mode}
            idPrefix="login"
            mode={mode}
            busy={busy}
            blockedReason={blocked}
            error={null}
            onSubmit={submit}
          />

          <div className="flex justify-center">
            <Button variant="ghost" onClick={leave}>Not now. Continue without an account</Button>
          </div>
        </div>
      </Screen>
    </PageRoot>
  );
}
