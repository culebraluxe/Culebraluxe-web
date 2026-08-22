// AUTH-01 — Auth.js v5 route handlers (signin / callback / signout / session).
// Portal protection is NOT activated in this story (AUTH-02); these routes are
// public so login and provider callbacks can complete.

import { handlers } from '@/auth'

export const { GET, POST } = handlers
