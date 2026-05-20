export {};

declare global {
  namespace YT {
    const PlayerState: {
      ENDED: number;
      PLAYING: number;
    };

    type PlayerStateEvent = {
      data: number;
    };

    class Player {
      constructor(
        elementId: string,
        options: {
          videoId?: string;
          playerVars?: Record<string, string | number>;
          events?: {
            onReady?: () => void;
            onStateChange?: (event: PlayerStateEvent) => void;
            onError?: () => void;
          };
        },
      );

      addEventListener(
        eventName: "onStateChange",
        listener: (event: PlayerStateEvent) => void,
      ): void;
      getCurrentTime(): number;
      getDuration(): number;
      getPlayerState(): number;
      loadVideoById(videoId: string): void;
      pauseVideo(): void;
      playVideo(): void;
      removeEventListener(
        eventName: "onStateChange",
        listener: (event: PlayerStateEvent) => void,
      ): void;
      seekTo(seconds: number, allowSeekAhead: boolean): void;
    }
  }

  interface Error {
    status?: number;
  }

  interface HTMLDivElement {
    _msgData?: unknown;
    _msgTime?: number;
    _replyParent?: HTMLDivElement | null;
  }

  interface Window {
    __moonmoonSync?: {
      generateToken: () => string;
      getToken: () => string;
      isValidToken: (token: unknown) => boolean;
      pull: () => Promise<unknown>;
      push: () => void;
      setToken: (token: string) => void;
    };
    onYouTubeIframeAPIReady?: () => void;
  }
}
