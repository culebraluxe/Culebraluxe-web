'use client'

import MuxPlayer from '@mux/mux-player-react'

import type { PropertyVideo } from '@/lib/property-types'

function playerAspectRatio(video: PropertyVideo) {
  if (!video.aspectRatio) {
    return video.role === 'short' ? '9 / 16' : '16 / 9'
  }

  return video.aspectRatio.replace(':', ' / ')
}

export function PropertyVideos({ videos }: { videos: PropertyVideo[] }) {
  const films = videos.filter((video) => video.role === 'video')
  const shorts = videos.filter((video) => video.role === 'short')

  return (
    <div className="flex flex-col gap-16">
      {films.length > 0 && (
        <section className="mx-auto w-full max-w-[860px]">
          <p className="mb-6 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
            Property Film
          </p>

          <div className="flex flex-col gap-10">
            {films.map((video) => (
              <div key={video.id}>
                <div
                  className="relative w-full overflow-hidden"
                  style={{
                    aspectRatio: playerAspectRatio(video),
                  }}
                >
                  <MuxPlayer
                    playbackId={video.playbackId}
                    metadata={{
                      video_title: video.title,
                    }}
                    style={{
                      position: 'absolute',
                      inset: 0,
                      display: 'block',
                      width: '100%',
                      height: '100%',
                      maxWidth: '100%',
                      maxHeight: '100%',
                    }}
                  />
                </div>

                {video.caption && (
                  <p className="mt-4 text-sm font-light leading-relaxed text-muted-foreground">
                    {video.caption}
                  </p>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      {shorts.length > 0 && (
        <section>
          <p className="mx-auto mb-6 w-full max-w-[1188px] text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">
            Short Films
          </p>

          <div className="flex flex-wrap justify-center gap-6">
            {shorts.map((video) => (
              <div key={video.id} className="w-full max-w-[380px]">
                <div
                  className="relative w-full overflow-hidden"
                  style={{
                    aspectRatio: playerAspectRatio(video),
                  }}
                >
                  <MuxPlayer
                    playbackId={video.playbackId}
                    metadata={{
                      video_title: video.title,
                    }}
                    style={{
                      position: 'absolute',
                      inset: 0,
                      display: 'block',
                      width: '100%',
                      height: '100%',
                      maxWidth: '100%',
                      maxHeight: '100%',
                    }}
                  />
                </div>

                {video.caption && (
                  <p className="mt-3 text-sm font-light leading-relaxed text-muted-foreground">
                    {video.caption}
                  </p>
                )}
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  )
}
