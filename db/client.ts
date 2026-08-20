import { neon } from "@neondatabase/serverless"

const appEnv = process.env.APP_ENV ?? "development"

export const databaseUrl =
  appEnv === "production"
    ? process.env.DATABASE_URL_PROD
    : process.env.DATABASE_URL_DEV

if (!databaseUrl) {
  throw new Error(
    `Database URL is not configured for APP_ENV="${appEnv}"`
  )
}

export const sql = neon(databaseUrl)