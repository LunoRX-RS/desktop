"use client";

import { useRef, useState, type FormEvent } from "react";
import { Button } from "../../components/ui";
import { FIELD, Inset, NoteRow, SettingRow } from "../settings/rows";

// Shared by Accounts and Sync. The password lives in state only until submit, then the field is cleared.
export function SignInForm({
  idPrefix,
  busy,
  blockedReason,
  error,
  onSubmit,
}: {
  idPrefix: string;
  busy: boolean;
  blockedReason?: string;
  error: string | null;
  onSubmit: (email: string, password: string) => void | Promise<void>;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [problems, setProblems] = useState<{ email?: string; password?: string }>({});
  const emailRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);
  const eId = `${idPrefix}-email`;
  const pId = `${idPrefix}-password`;
  const errId = `${idPrefix}-error`;

  function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || blockedReason) return;
    const next: { email?: string; password?: string } = {};
    if (email.trim() === "") next.email = "Enter your email address.";
    else if (!/^\S+@\S+$/.test(email.trim())) next.email = "Enter an email address like name@example.com.";
    if (password === "") next.password = "Enter your password.";
    setProblems(next);
    if (next.email) return void emailRef.current?.focus();
    if (next.password) return void passwordRef.current?.focus();
    const secret = password;
    setPassword(""); // not kept in state past submit
    void onSubmit(email.trim(), secret);
  }

  const describe = (own: string | undefined, ownId: string) =>
    [own ? ownId : "", error ? errId : ""].filter(Boolean).join(" ") || undefined;

  return (
    <form onSubmit={submit} noValidate aria-busy={busy}>
      <Inset id={`${idPrefix}-title`} title="Sign in">
        <SettingRow label="Email" htmlFor={eId}>
          <input
            ref={emailRef}
            id={eId}
            type="email"
            autoComplete="username"
            value={email}
            disabled={busy}
            aria-invalid={problems.email ? true : undefined}
            aria-describedby={describe(problems.email, `${eId}-problem`)}
            onChange={(e) => setEmail(e.currentTarget.value)}
            className={`${FIELD} w-[280px]`}
          />
        </SettingRow>
        {problems.email && (
          <NoteRow tone="bad" id={`${eId}-problem`} role="alert">
            {problems.email}
          </NoteRow>
        )}
        <SettingRow label="Password" htmlFor={pId}>
          <input
            ref={passwordRef}
            id={pId}
            type="password"
            autoComplete="current-password"
            value={password}
            disabled={busy}
            aria-invalid={problems.password ? true : undefined}
            aria-describedby={describe(problems.password, `${pId}-problem`)}
            onChange={(e) => setPassword(e.currentTarget.value)}
            className={`${FIELD} w-[280px]`}
          />
        </SettingRow>
        {problems.password && (
          <NoteRow tone="bad" id={`${pId}-problem`} role="alert">
            {problems.password}
          </NoteRow>
        )}
        {error && (
          <NoteRow tone="bad" id={errId} role="alert">
            {error}
          </NoteRow>
        )}
        <SettingRow label="LUNOR-X account" hint={blockedReason ?? "Press Enter to sign in."} hintId={`${idPrefix}-hint`}>
          <Button type="submit" variant="primary" disabled={busy || !!blockedReason} aria-describedby={`${idPrefix}-hint`}>
            {busy ? "Signing in…" : "Sign in"}
          </Button>
        </SettingRow>
      </Inset>
    </form>
  );
}
