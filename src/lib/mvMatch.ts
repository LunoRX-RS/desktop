// A YouTube video id is 11 URL-safe characters.
export const isVideoId = (id: string) => /^[A-Za-z0-9_-]{11}$/.test(id);
