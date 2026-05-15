# E702: Invalid MOR chunk format - missing |

## Description

Invalid MOR chunk format - missing |

## Metadata
- **Status**: not_implemented

- **Error Code**: E702
- **Category**: Dependent tier parsing
- **Level**: tier
- **Layer**: parser

## Example

```chat
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	hello n|world .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: %mor chunk without pipe separator

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). Each tier type has specific syntax requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus file: `error_corpus/E7xx_tier_parsing/E702_invalid_mor_format.cha`
- Review and enhance this specification as needed
