"use client";

import * as React from "react";
import {
  Armchair,
  CircleAlert,
  Eye,
  EyeOff,
  Loader2,
} from "lucide-react";

import { useSession } from "@/components/session/session-provider";
import { authLogin, shopLogoGet } from "@/lib/tauri/api";
import type { CommandError } from "@/lib/tauri/client";

export function LoginScreen() {
  const { applyLogin } = useSession();
  const [username, setUsername] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [showPassword, setShowPassword] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [waitSecs, setWaitSecs] = React.useState(0);
  const [logo, setLogo] = React.useState<string | null>(null);
  const usernameRef = React.useRef<HTMLInputElement>(null);
  const passwordRef = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    let active = true;

    void shopLogoGet("").then((res) => {
      if (active && res) setLogo(res);
    }).catch(() => {});

    return () => {
      active = false;
    };
  }, []);

  React.useEffect(() => {
    if (waitSecs <= 0) return;
    const timer = window.setTimeout(() => setWaitSecs((seconds) => seconds - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [waitSecs]);

  function clearError() {
    if (waitSecs === 0) setError(null);
  }

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || waitSecs > 0) return;

    const cleanUsername = username.trim();
    if (!cleanUsername) {
      setError("Enter your username.");
      usernameRef.current?.focus();
      return;
    }
    if (!password) {
      setError("Enter your password.");
      passwordRef.current?.focus();
      return;
    }

    setBusy(true);
    setError(null);
    try {
      const result = await authLogin(cleanUsername, password);
      applyLogin(result);
    } catch (caught) {
      const commandError = caught as CommandError;
      if (commandError.code === "RATE_LIMITED" && commandError.retryAfterSecs) {
        setWaitSecs(commandError.retryAfterSecs);
        setError(`Too many failed attempts. Try again in ${commandError.retryAfterSecs} seconds.`);
      } else if (commandError.code === "INVALID_CREDENTIALS") {
        setError("Invalid username or password.");
        passwordRef.current?.focus();
        passwordRef.current?.select();
      } else {
        setError(commandError.message || "Unable to sign in. Please try again.");
      }
    } finally {
      setBusy(false);
    }
  }

  const controlsDisabled = busy || waitSecs > 0;

  return (
    <main className="flex min-h-screen flex-col items-center justify-center bg-[#f3f5f8] px-4 py-8 text-[#172033] sm:px-6">
      <section
        aria-labelledby="login-title"
        className="w-full max-w-[460px] rounded-2xl border border-[#e2e6ec] bg-white px-6 py-8 shadow-[0_8px_28px_rgba(23,42,76,0.08)] sm:px-10 sm:py-10"
      >
        <header className="text-center">
          {logo ? (
            <img src={logo} alt="Showroom Logo" className="mx-auto h-16 w-auto object-contain" />
          ) : (
            <span className="mx-auto flex h-14 w-14 items-center justify-center rounded-xl bg-[#192f58] text-white shadow-sm">
              <Armchair className="h-7 w-7" strokeWidth={1.8} aria-hidden="true" />
            </span>
          )}
          <h1 id="login-title" className="mt-5 text-[22px] font-semibold tracking-[-0.02em] text-[#172033]">
            Furniture Showroom Manager
          </h1>
          <p className="mt-1.5 text-sm text-[#8a94a6]">Professional Furniture Management</p>
        </header>

        <div className="my-8 h-px bg-[#e8ebef]" aria-hidden="true" />

        <div className="mb-7 text-center">
          <h2 className="text-2xl font-semibold tracking-[-0.02em] text-[#172033]">Welcome back</h2>
          <p className="mt-1.5 text-[15px] text-[#7f899b]">Sign in to continue</p>
        </div>

        <form onSubmit={submit} noValidate className="space-y-5">
          <div>
            <label htmlFor="login-username" className="mb-2 block text-sm font-medium text-[#3c4658]">
              Username <span className="text-red-500">*</span>
            </label>
            <input
              ref={usernameRef}
              id="login-username"
              name="username"
              value={username}
              onChange={(event) => {
                setUsername(event.target.value);
                clearError();
              }}
              autoComplete="username"
              autoCapitalize="none"
              spellCheck={false}
              autoFocus
              disabled={controlsDisabled}
              aria-invalid={Boolean(error)}
              aria-describedby={error ? "login-error" : undefined}
              placeholder="Enter your username"
              className="h-12 w-full rounded-lg border border-[#cfd5df] bg-white px-4 text-[15px] text-[#172033] shadow-sm outline-none transition placeholder:text-[#a5adba] hover:border-[#aeb8c7] focus:border-[#315f9d] focus:ring-4 focus:ring-[#315f9d]/10 disabled:cursor-not-allowed disabled:bg-[#f6f7f9] disabled:text-[#7f899b]"
            />
          </div>

          <div>
            <label htmlFor="login-password" className="mb-2 block text-sm font-medium text-[#3c4658]">
              Password <span className="text-red-500">*</span>
            </label>
            <div className="relative">
              <input
                ref={passwordRef}
                id="login-password"
                name="password"
                type={showPassword ? "text" : "password"}
                value={password}
                onChange={(event) => {
                  setPassword(event.target.value);
                  clearError();
                }}
                autoComplete="current-password"
                disabled={controlsDisabled}
                aria-invalid={Boolean(error)}
                aria-describedby={error ? "login-error" : undefined}
                placeholder="Enter your password"
                className="h-12 w-full rounded-lg border border-[#cfd5df] bg-white py-2 pl-4 pr-12 text-[15px] text-[#172033] shadow-sm outline-none transition placeholder:text-[#a5adba] hover:border-[#aeb8c7] focus:border-[#315f9d] focus:ring-4 focus:ring-[#315f9d]/10 disabled:cursor-not-allowed disabled:bg-[#f6f7f9] disabled:text-[#7f899b]"
              />
              <button
                type="button"
                onClick={() => setShowPassword((visible) => !visible)}
                disabled={controlsDisabled}
                aria-label={showPassword ? "Hide password" : "Show password"}
                aria-pressed={showPassword}
                aria-controls="login-password"
                className="absolute inset-y-0 right-0 flex w-12 items-center justify-center rounded-r-lg text-[#8a94a6] outline-none transition hover:text-[#315f9d] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#315f9d] disabled:cursor-not-allowed disabled:opacity-50"
              >
                {showPassword ? (
                  <EyeOff className="h-5 w-5" aria-hidden="true" />
                ) : (
                  <Eye className="h-5 w-5" aria-hidden="true" />
                )}
              </button>
            </div>
          </div>

          {error && (
            <div
              id="login-error"
              role="alert"
              className="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2.5 text-sm text-red-700"
            >
              <CircleAlert className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
              <span>{error}</span>
            </div>
          )}

          <button
            type="submit"
            disabled={controlsDisabled}
            className="flex h-12 w-full items-center justify-center gap-2 rounded-lg bg-[#192f58] px-4 text-[15px] font-semibold text-white shadow-sm outline-none transition hover:bg-[#132746] focus-visible:ring-4 focus-visible:ring-[#315f9d]/25 disabled:cursor-not-allowed disabled:bg-[#71809a]"
          >
            {busy ? (
              <>
                <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
                Signing in…
              </>
            ) : waitSecs > 0 ? (
              `Try again in ${waitSecs}s`
            ) : (
              "Sign In"
            )}
          </button>

          <p className="pt-1 text-center text-xs text-[#8a94a6]">
            Powered by <strong className="font-semibold text-[#3c4658]">EagleNest Creations</strong> (0346-4451505)
          </p>
        </form>
      </section>
    </main>
  );
}
