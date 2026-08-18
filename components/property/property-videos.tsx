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
          <p className="mb-6 text-xs font-medium uppercase tracking-[0.24em] text-[#c6a15b]">
            Property Film
          </p>

          <div className="flex flex-col gap-10">
            {films.map((video) => (
              <div key={video.id}>
                <div
                  className="relative w-full overflow-hidden border border-brand-navy/45 bg-brand-navy shadow-[0_12px_30px_rgba(3,15,35,0.08)]"
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
                  <p className="mt-4 text-sm font-normal leading-relaxed text-brand-navy/85">
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
          <p className="mx-auto mb-6 w-full max-w-[1188px] text-xs font-medium uppercase tracking-[0.24em] text-[#c6a15b]">
            Short Films
          </p>

          <div className="flex flex-wrap justify-center gap-6">
            {shorts.map((video) => (
              <div key={video.id} className="w-full max-w-[380px]">
                <div
                  className="relative w-full overflow-hidden border border-brand-navy/45 bg-brand-navy shadow-[0_10px_24px_rgba(3,15,35,0.07)]"
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
                  <p className="mt-3 text-sm font-normal leading-relaxed text-brand-navy/85">
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
