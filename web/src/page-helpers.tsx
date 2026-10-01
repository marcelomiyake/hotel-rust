import { ApiError } from "./api";

export function PageHeading({ eyebrow, title, lede }: Readonly<{ eyebrow: string; title: string; lede: string }>) {
  return <header className="page-heading-row"><div><p className="eyebrow">{eyebrow}</p><h1>{title}</h1><p>{lede}</p></div></header>;
}

export function friendlyError(error: unknown): string {
  if (error instanceof ApiError && error.status === 404) return "This stay or reservation could not be found.";
  if (error instanceof ApiError && error.status === 401) return "That staff token was not accepted. Check the token and try again.";
  if (error instanceof ApiError && error.status === 409) return "That room was just reserved. Refresh availability and choose another option.";
  if (error instanceof ApiError) return error.message;
  return "We could not reach the reservation service. Please try again in a moment.";
}
