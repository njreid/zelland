❯ Ok, let's focus just on the Markdown Preview. I want to be able to add and view annotations to the file. Let's design the front-end experience first.

# In the document

❯ Ok, let's focus just on the Markdown Preview. I want to be able to add and view annotations to the file. Let's design the front-end experience first.
  Anchors for annotations should appear as blue underlines under anchor text, the text itself should also have a faint blue highlight. # Desktop

## Desktop

- If annotations exist,
  show a closable right-hand sidebar containing all comment threads (chain) based on the order they appear in the document.

- Clicking on an anchor causes the annotation sidebar to fast-scroll to the top of the linked comment chain.

- The user can add an annotation by selecting text in the main document. This should
  cause the annotation sidebar to scroll to the appropriate slot and

## Mobile

- Clicking on an annotation should expand a comment view _inline_ in the document.

- By default, only the last comment in a chain is visible, along with a small reply box which expands when the user clicks on it. Replies can be submitted or canceled.

- User can choose to expand all of the previous comments in the chain above the last reply.
- There is also a way to close the whole chain.

## Annotation Format

- Each comment within the chain should have a small subtle indicator of how long ago the comment was last edited, with the comment author name.

- Comments can contain markdown formatting.
