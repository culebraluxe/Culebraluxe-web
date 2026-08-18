export type GoogleMapInstance = object

export type GoogleMapsNamespace = {
  Map: new (
    element: HTMLElement,
    options: Record<string, unknown>,
  ) => GoogleMapInstance
  Marker: new (options: Record<string, unknown>) => {
    setMap: (map: GoogleMapInstance | null) => void
  }
  Size: new (width: number, height: number) => unknown
  Point: new (x: number, y: number) => unknown
  MapTypeId: {
    ROADMAP: string
    SATELLITE: string
    HYBRID: string
    TERRAIN: string
  }
  MapTypeControlStyle: {
    DROPDOWN_MENU: number
  }
  marker: {
    AdvancedMarkerElement: new (
      options: Record<string, unknown>,
    ) => HTMLElement & { map: GoogleMapInstance | null }
    PinElement: new (options: Record<string, unknown>) => HTMLElement
  }
  event: {
    clearInstanceListeners: (instance: object) => void
  }
}

type GoogleMapsWindow = Window & {
  google?: {
    maps?: GoogleMapsNamespace
  }
  __culebraLuxeGoogleMapsReady?: () => void
}

const SCRIPT_ID = 'culebraluxe-google-maps-js'
const CALLBACK = '__culebraLuxeGoogleMapsReady'

let loaderPromise: Promise<GoogleMapsNamespace> | null = null

export function loadGoogleMaps(apiKey: string) {
  const googleWindow = window as GoogleMapsWindow

  if (googleWindow.google?.maps?.marker) {
    return Promise.resolve(googleWindow.google.maps)
  }

  if (loaderPromise) return loaderPromise

  loaderPromise = new Promise<GoogleMapsNamespace>((resolve, reject) => {
    googleWindow[CALLBACK] = () => {
      const maps = googleWindow.google?.maps

      if (!maps?.marker) {
        loaderPromise = null
        reject(new Error('Google Maps marker library did not load.'))
        return
      }

      resolve(maps)
      delete googleWindow[CALLBACK]
    }

    if (!document.getElementById(SCRIPT_ID)) {
      const scriptUrl = new URL('https://maps.googleapis.com/maps/api/js')
      scriptUrl.searchParams.set('key', apiKey)
      scriptUrl.searchParams.set('v', 'weekly')
      scriptUrl.searchParams.set('loading', 'async')
      scriptUrl.searchParams.set('libraries', 'marker')
      scriptUrl.searchParams.set('callback', CALLBACK)

      const script = document.createElement('script')
      script.id = SCRIPT_ID
      script.src = scriptUrl.toString()
      script.async = true
      script.addEventListener(
        'error',
        () => {
          loaderPromise = null
          reject(new Error('Google Maps JavaScript API could not load.'))
        },
        { once: true },
      )
      document.head.appendChild(script)
    }
  })

  return loaderPromise
}
