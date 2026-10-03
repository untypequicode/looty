import { useMediaQuery } from '@vueuse/core'

/** Points de rupture partagés : mobile < 640px, tablette < 1024px. */
export function useBreakpoint() {
  const isMobile = useMediaQuery('(max-width: 639px)')
  const isTablet = useMediaQuery('(max-width: 1023px)')
  const isTouch = useMediaQuery('(pointer: coarse)')
  return { isMobile, isTablet, isTouch }
}
