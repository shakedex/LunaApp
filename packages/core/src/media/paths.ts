/** root/rel when the scanned root is known; rel alone otherwise (browsers
 *  cannot see the absolute disk path — the root folder name is the deepest
 *  honest prefix). */
export function joinPath(root: string, relative: string): string {
  return root ? `${root}/${relative}` : relative
}
