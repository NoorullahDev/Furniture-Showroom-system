"use client";

import * as React from "react";

/**
 * Route protection gate.
 *
 * Phase 1 has no authentication yet, so this gate is a pass-through. Phase 2
 * will replace it with a session/permission check backed by
 * `auth.current_session` and route-level permission codes; every page that
 * needs protection must then be wrapped in this component.
 */
export function RouteGuard({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}