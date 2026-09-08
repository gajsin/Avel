import { ClipboardItem } from '../types';

export interface SplitClipboardItems {
  imageItems: ClipboardItem[];
  textItems: ClipboardItem[];
  isOnlyImages: boolean;
  isOnlyTexts: boolean;
  isMixed: boolean;
}

export function splitSelectedClipboardItems(items: ClipboardItem[]): SplitClipboardItems {
  const imageItems = items.filter(
    (i) => (i.content_type === 'image' || i.content_type === 'screenshot') && Boolean(i.image_path)
  );
  const textItems = items.filter(
    (i) => i.content_type !== 'image' && i.content_type !== 'screenshot' && Boolean(i.text_content)
  );

  return {
    imageItems,
    textItems,
    isOnlyImages: imageItems.length > 0 && textItems.length === 0,
    isOnlyTexts: textItems.length > 0 && imageItems.length === 0,
    isMixed: imageItems.length > 0 && textItems.length > 0,
  };
}
