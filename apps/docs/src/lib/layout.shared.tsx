import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";
import Image from "next/image";

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <span className="inline-flex items-center gap-2 font-semibold">
          <Image
            src="/brand/still-mark.svg"
            width={28}
            height={28}
            alt=""
            className="h-7 w-7"
          />
          Still
        </span>
      ),
    },
  };
}
