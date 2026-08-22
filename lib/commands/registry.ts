// ---------------------------------------------------------------------------
// CRM-14J — In-memory command registry.
//
// Maps a stable command type to its handler. Registration is the ONLY thing a
// new business command needs beyond its thin handler: define payload/result
// types, implement the handler, register it here, add targeted tests. No
// replay/transaction/routing/correlation/audit behavior needs inventing again.
// ---------------------------------------------------------------------------

import type {
  CommandHandler,
  CommandRegistry,
} from './contracts'

export class InMemoryCommandRegistry implements CommandRegistry {
  private readonly handlers = new Map<string, CommandHandler<any, any>>()

  register(commandType: string, handler: CommandHandler<any, any>): void {
    if (this.handlers.has(commandType)) {
      throw new Error(`Command type already registered: ${commandType}`)
    }
    this.handlers.set(commandType, handler)
  }

  resolve(commandType: string): CommandHandler<any, any> | undefined {
    return this.handlers.get(commandType)
  }

  /** Registered command types (for inventory/tests). */
  list(): string[] {
    return [...this.handlers.keys()].sort()
  }
}
