"use client"

import MuxPlayer from "@mux/mux-player-react"

export default function VideoPage() {
  return (
    <main className="min-h-screen bg-black p-8">
      <div className="mx-auto max-w-6xl">
        <MuxPlayer
          playbackId="PpZmyDr01700pC5WKZybM6OPrDn4JK01bvqQLKWrdhO514"
          metadata={{
            video_title: "Navillus_Sur2",
          }}
          style={{
            width: "100%",
            aspectRatio: "16 / 9",
          }}
        />
      </div>
    </main>
  )
}