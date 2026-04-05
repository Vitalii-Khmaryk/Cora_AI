import { FC } from 'react';
import './EyeSphere.css';
import { CoraState } from '../../types';
import { irisGroups } from './irisGroups';

interface EyeSphereProps {
    coraState: CoraState;
}

let gradientIndex = 3;
const groups = irisGroups.map((group) => ({
    ...group,
    lines: group.lines.map((line) => {
        const id = `paint${gradientIndex++}_linear_316_1580`;
        return { id, pathOpacity: line[0], d: line[1] as string, x1: line[2] as number, y1: line[3] as number, x2: line[4] as number, y2: line[5] as number };
    }),
}));

const EyeSphere: FC<EyeSphereProps> = ({ coraState }) => {
    return (
        <div className={`cora-eye-wrapper state-${coraState}`}>
            <svg viewBox="0 0 240 240" fill="none" xmlns="http://www.w3.org/2000/svg">
                <g filter="url(#filter0_d_316_1580)">
                    {/* Blurred angular gradient glow – outer */}
                    <g filter="url(#filter1_f_316_1580)">
                        <g clipPath="url(#paint0_angular_316_1580_clip_path)" data-figma-skip-parse="true">
                            <g transform="matrix(0 0.1 -0.1 0 120 120)">
                                <foreignObject>
                                    <div style={{ background: 'conic-gradient(from 90deg,rgba(123, 41, 15, 1) 0deg,rgba(173, 102, 50, 1) 112.92deg,rgba(255, 134, 23, 1) 360deg)', height: '100%', width: '100%', opacity: 0.4 }} />
                                </foreignObject>
                            </g>
                        </g>
                        <circle cx="120" cy="120" r="100" />
                    </g>

                    {/* Blurred angular gradient glow – inner */}
                    <g filter="url(#filter2_f_316_1580)">
                        <g clipPath="url(#paint1_angular_316_1580_clip_path)" data-figma-skip-parse="true">
                            <g transform="matrix(0 0.1 -0.1 0 120 120)">
                                <foreignObject>
                                    <div style={{ background: 'conic-gradient(from 90deg,rgba(123, 41, 15, 1) 0deg,rgba(173, 102, 50, 1) 112.92deg,rgba(255, 134, 23, 1) 360deg)', height: '100%', width: '100%', opacity: 0.8 }} />
                                </foreignObject>
                            </g>
                        </g>
                        <circle cx="120" cy="120" r="100" />
                    </g>

                    {/* Dark pupil with inner shadow */}
                    <g filter="url(#filter3_iii_316_1580)">
                        <circle cx="120" cy="120" r="100" fill="#1E100B" className="cora-pupil" />
                    </g>

                    {/* Iris lines */}
                    <mask id="mask0_316_1580" style={{ maskType: 'alpha' }} maskUnits="userSpaceOnUse">
                        <circle cx="120" cy="120" r="100" fill="url(#paint2_radial_316_1580)" />
                    </mask>
                    <g mask="url(#mask0_316_1580)">
                        <mask id="mask1_316_1580" style={{ maskType: 'alpha' }} maskUnits="userSpaceOnUse">
                            <circle cx="120" cy="120" r="100" fill="#C4C4C4" />
                        </mask>
                        <g mask="url(#mask1_316_1580)" className="cora-iris-lines">
                            {groups.map((group, groupIdx) => (
                                <g key={groupIdx} opacity={group.groupOpacity}>
                                    {group.lines.map(({ id, pathOpacity, d }) => (
                                        <path key={id} opacity={pathOpacity} d={d} stroke={`url(#${id})`} strokeLinecap="square" />
                                    ))}
                                </g>
                            ))}

                            {/* Highlight glows */}
                            <g filter="url(#filter4_f_316_1580)" style={{ mixBlendMode: 'plus-lighter' }}>
                                <path d="M136.379 36.8199C153.952 54.3935 107.606 45.5932 80.6654 72.5334C53.7252 99.4736 45.6396 141.199 38.8198 134.379C32 127.559 28.8393 80.7197 55.7795 53.7796C82.7197 26.8394 118.805 19.2463 136.379 36.8199Z" fill="#ECBB98" />
                            </g>
                            <g filter="url(#filter5_f_316_1580)" style={{ mixBlendMode: 'plus-lighter' }}>
                                <path d="M110.857 204.416C93.2835 186.843 139.63 195.643 166.571 168.703C193.511 141.762 201.596 100.037 208.416 106.857C215.236 113.677 218.397 160.516 191.456 187.456C164.516 214.397 128.431 221.99 110.857 204.416Z" fill="#ECBB98" />
                            </g>
                        </g>
                    </g>
                </g>

                <defs>
                    {/* ── Filters ── */}
                    <filter id="filter0_d_316_1580" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
                        <feFlood floodOpacity="0" result="BackgroundImageFix" />
                        <feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha" />
                        <feOffset dy="4" />
                        <feGaussianBlur stdDeviation="2" />
                        <feComposite in2="hardAlpha" operator="out" />
                        <feColorMatrix type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.25 0" />
                        <feBlend mode="normal" in2="BackgroundImageFix" result="effect1_dropShadow_316_1580" />
                        <feBlend mode="normal" in="SourceGraphic" in2="effect1_dropShadow_316_1580" result="shape" />
                    </filter>
                    <filter id="filter1_f_316_1580" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
                        <feFlood floodOpacity="0" result="BackgroundImageFix" />
                        <feBlend mode="normal" in="SourceGraphic" in2="BackgroundImageFix" result="shape" />
                        <feGaussianBlur stdDeviation="10" result="effect1_foregroundBlur_316_1580" />
                    </filter>
                    <filter id="filter2_f_316_1580" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
                        <feFlood floodOpacity="0" result="BackgroundImageFix" />
                        <feBlend mode="normal" in="SourceGraphic" in2="BackgroundImageFix" result="shape" />
                        <feGaussianBlur stdDeviation="2.5" result="effect1_foregroundBlur_316_1580" />
                    </filter>
                    <filter id="filter3_iii_316_1580" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
                        <feFlood floodOpacity="0" result="BackgroundImageFix" />
                        <feBlend mode="normal" in="SourceGraphic" in2="BackgroundImageFix" result="shape" />
                        <feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha" />
                        <feOffset /><feGaussianBlur stdDeviation="20" />
                        <feComposite in2="hardAlpha" operator="arithmetic" k2="-1" k3="1" />
                        <feColorMatrix type="matrix" values="0 0 0 0 0.650917 0 0 0 0 0.343292 0 0 0 0 0.24075 0 0 0 1 0" />
                        <feBlend mode="normal" in2="shape" result="effect1_innerShadow_316_1580" />
                        <feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha" />
                        <feOffset /><feGaussianBlur stdDeviation="15" />
                        <feComposite in2="hardAlpha" operator="arithmetic" k2="-1" k3="1" />
                        <feColorMatrix type="matrix" values="0 0 0 0 0.73 0 0 0 0 0.385 0 0 0 0 0.27 0 0 0 1 0" />
                        <feBlend mode="normal" in2="effect1_innerShadow_316_1580" result="effect2_innerShadow_316_1580" />
                        <feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha" />
                        <feOffset /><feGaussianBlur stdDeviation="6" />
                        <feComposite in2="hardAlpha" operator="arithmetic" k2="-1" k3="1" />
                        <feColorMatrix type="matrix" values="0 0 0 0 0.929125 0 0 0 0 0.838562 0 0 0 0 0.808375 0 0 0 1 0" />
                        <feBlend mode="normal" in2="effect2_innerShadow_316_1580" result="effect3_innerShadow_316_1580" />
                    </filter>
                    <filter id="filter4_f_316_1580" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
                        <feFlood floodOpacity="0" result="BackgroundImageFix" />
                        <feBlend mode="normal" in="SourceGraphic" in2="BackgroundImageFix" result="shape" />
                        <feGaussianBlur stdDeviation="20" result="effect1_foregroundBlur_316_1580" />
                    </filter>
                    <filter id="filter5_f_316_1580" filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
                        <feFlood floodOpacity="0" result="BackgroundImageFix" />
                        <feBlend mode="normal" in="SourceGraphic" in2="BackgroundImageFix" result="shape" />
                        <feGaussianBlur stdDeviation="20" result="effect1_foregroundBlur_316_1580" />
                    </filter>

                    {/* ── Clip paths ── */}
                    <clipPath id="paint0_angular_316_1580_clip_path"><circle cx="120" cy="120" r="100" /></clipPath>
                    <clipPath id="paint1_angular_316_1580_clip_path"><circle cx="120" cy="120" r="100" /></clipPath>

                    {/* ── Radial gradient for iris mask ── */}
                    <radialGradient id="paint2_radial_316_1580" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(120 120) rotate(90) scale(100)">
                        <stop offset="0.195852" stopColor="#C4C4C4" stopOpacity="0" />
                        <stop offset="0.267419" stopColor="#C4C4C4" />
                        <stop offset="0.775733" stopColor="#C4C4C4" />
                        <stop offset="1" stopColor="#C4C4C4" stopOpacity="0" />
                    </radialGradient>

                    {/* ── Linear gradients (generated from irisGroups data) ── */}
                    {groups.map(group =>
                        group.lines.map(({ id, x1, y1, x2, y2 }) => (
                            <linearGradient key={id} id={id} x1={x1} y1={y1} x2={x2} y2={y2} gradientUnits="userSpaceOnUse">
                                <stop offset="0.442708" stopColor="#FFCCA8" />
                                <stop offset="0.5" stopColor="white" stopOpacity="0" />
                                <stop offset="1" stopColor="white" stopOpacity="0" />
                            </linearGradient>
                        ))
                    )}
                </defs>
            </svg>
        </div>
    );
};

export default EyeSphere;