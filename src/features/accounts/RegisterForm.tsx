"use client";

import { useRef, useState, type FormEvent } from "react";
import { Button } from "../../components/ui";
import { FIELD, Inset, NoteRow, SettingRow } from "../settings/rows";

// The server counts bytes, not characters: a Thai character is three bytes. Same limits as the backend.
const MIN_BYTES = 12;
const MAX_BYTES = 128;
const bytes = (text: string) => new TextEncoder().encode(text).length;
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/** Create an account. The password lives in state only until submit, then both fields are cleared. */
export function RegisterForm({
  idPrefix,
  busy,
  error,
  onSubmit,
  onHaveAccount,
}: {
  idPrefix: string;
  busy: boolean;
  error: string | null;
  onSubmit: (email: string, password: string) => void | Promise<void>;
  onHaveAccount: () => void;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [problems, setProblems] = useState<{ email?: string; password?: string; confirm?: string }>({});
  const emailRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);
  const confirmRef = useRef<HTMLInputElement>(null);
  const eId = `${idPrefix}-email`;
  const pId = `${idPrefix}-password`;
  const cId = `${idPrefix}-confirm`;
  const errId = `${idPrefix}-error`;
  const size = bytes(password);
  const sizeOk = size >= MIN_BYTES && size <= MAX_BYTES;

  function submit(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    const next: { email?: string; password?: string; confirm?: string } = {};
    if (!EMAIL.test(email.trim())) next.email = "Enter an email address like name@example.com.";
    if (!sizeOk) next.password = `Use ${MIN_BYTES} to ${MAX_BYTES} bytes. Thai and other non-Latin characters take more than one byte each.`;
    if (confirm !== password) next.confirm = "The two passwords do not match.";
    setProblems(next);
    if (next.email) return void emailRef.current?.focus();
    if (next.password) return void passwordRef.current?.focus();
    if (next.confirm) return void confirmRef.current?.focus();
    const secret = password;
    setPassword("");
    setConfirm("");
    void onSubmit(email.trim(), secret);
  }

  const describe = (own: string | undefined, ownId: string) =>
    [own ? ownId : "", error ? errId : ""].filter(Boolean).join(" ") || undefined;

  return (
    <form onSubmit={submit} noValidate aria-busy={busy}>
      <Inset id={`${idPrefix}-title`} title="Create account">
        <SettingRow label="Email" htmlFor={eId}>
          <input
            ref={emailRef}
            id={eId}
            type="email"
            autoComplete="email"
            value={email}
            disabled={busy}
            aria-invalid={problems.email ? true : undefined}
            aria-describedby={describe(problems.email, `${eId}-problem`)}
            onChange={(e) => setEmail(e.currentTarget.value)}
            className={`${FIELD} w-[280px]`}
          />
        </SettingRow>
        {problems.email && <NoteRow tone="bad" id={`${eId}-problem`} role="alert">{problems.email}</NoteRow>}
        <SettingRow label="Password" htmlFor={pId} hint={`${size} of ${MAX_BYTES} bytes · at least ${MIN_BYTES}`}>
          <input
            ref={passwordRef}
            id={pId}
            type="password"
            autoComplete="new-password"
            value={password}
            disabled={busy}
            aria-invalid={problems.password ? true : undefined}
            aria-describedby={describe(problems.password, `${pId}-problem`)}
            onChange={(e) => setPassword(e.currentTarget.value)}
            className={`${FIELD} w-[280px]`}
          />
        </SettingRow>
        {problems.password && <NoteRow tone="bad" id={`${pId}-problem`} role="alert">{problems.password}</NoteRow>}
        <SettingRow label="Confirm password" htmlFor={cId}>
          <input
            ref={confirmRef}
            id={cId}
            type="password"
            autoComplete="new-password"
            value={confirm}
            disabled={busy}
            aria-invalid={problems.confirm ? true : undefined}
            aria-describedby={describe(problems.confirm, `${cId}-problem`)}
            onChange={(e) => setConfirm(e.currentTarget.value)}
            className={`${FIELD} w-[280px]`}
          />
        </SettingRow>
        {problems.confirm && <NoteRow tone="bad" id={`${cId}-problem`} role="alert">{problems.confirm}</NoteRow>}
        {error && <NoteRow tone="bad" id={errId} role="alert">{error}</NoteRow>}
        <SettingRow label="LunoRX account" hint="Passwords go only to the LunoRX backend.">
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? "Creating account…" : "Create account"}
          </Button>
        </SettingRow>
      </Inset>
      <div className="mt-3 flex justify-center">
        <Button variant="ghost" disabled={busy} onClick={onHaveAccount}>I already have an account</Button>
      </div>
    </form>
  );
}
